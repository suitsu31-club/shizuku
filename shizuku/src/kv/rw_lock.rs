use crate::kv::{KeyValue, KeyValueRead, KeyValueWrite, KvReadError, KvWriteError};
use async_nats::jetstream::kv::{CreateErrorKind, Store};
use rand::Rng;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use kanau::message::{DeserializeError, SerializeError};
use kanau::processor::Processor;
use kanau_macro::{BincodeMessageDe, BincodeMessageSer};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, BincodeMessageDe, BincodeMessageSer, bincode::Encode, bincode::Decode)]
/// The Kernel of [DistroRwLock]
pub struct DistroRwLockValue {
    /// The current mode of the lock.
    pub mode: DistroRwLockMode,

    /// The number of readers.
    pub readers: u64,

    /// Whether there is a writer waiting.
    pub writer_waiting: bool,

    /// Timestamp when the lock was acquired (in seconds since UNIX epoch).
    /// Only relevant when mode is Read or Write.
    pub acquired_at: u64,
}

impl DistroRwLockValue {
    /// Create a new lock.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the current timestamp in seconds since UNIX epoch.
    fn current_timestamp() -> u64 {
        #[allow(clippy::expect_used)]
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs()
    }

    /// Check if the lock has been held for more than 60 seconds.
    pub fn is_expired(&self) -> bool {
        if self.mode == DistroRwLockMode::Idle {
            return false;
        }

        let now = Self::current_timestamp();
        now.saturating_sub(self.acquired_at) > 60
    }

    /// Update the lock into a read acquired state.
    #[inline]
    pub fn into_read_acquired(self) -> Self {
        Self {
            mode: DistroRwLockMode::Read,
            readers: self.readers + 1,
            writer_waiting: self.writer_waiting,
            acquired_at: Self::current_timestamp(),
        }
    }

    /// Update the lock into a read released state.
    #[inline]
    pub fn into_read_released(self) -> Self {
        let new_readers = self.readers - 1;
        Self {
            mode: if new_readers > 0 {
                DistroRwLockMode::Read
            } else {
                DistroRwLockMode::Idle
            },
            readers: new_readers,
            writer_waiting: self.writer_waiting,
            acquired_at: 0,
        }
    }

    /// Update whether there is a writer waiting.
    #[inline]
    pub fn into_waiting(self, writer_waiting: bool) -> Self {
        Self {
            mode: self.mode,
            readers: self.readers,
            writer_waiting,
            acquired_at: self.acquired_at,
        }
    }

    /// Update the lock into a write acquired state.
    #[inline]
    pub fn into_write_acquired(self) -> Self {
        Self {
            mode: DistroRwLockMode::Write,
            readers: 0,
            writer_waiting: false,
            acquired_at: Self::current_timestamp(),
        }
    }

    /// Update the lock into a write released state.
    pub fn into_write_released(self) -> Self {
        Self {
            mode: DistroRwLockMode::Idle,
            readers: 0,
            writer_waiting: self.writer_waiting,
            acquired_at: 0,
        }
    }
}

/// The current mode of the lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, BincodeMessageDe, BincodeMessageSer, bincode::Encode, bincode::Decode)]
pub enum DistroRwLockMode {
    /// the lock is held by a reader
    Read,

    /// the lock is held by a writer
    Write,

    #[default]
    /// the lock is not held
    Idle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
/// Error when deserializing the lock state.
pub enum DistroRwLockDesErr {
    /// the length of the bytes is not 9
    #[error("Invalid byte length")]
    InvalidByteLength,


    /// the first byte is not `0b000`, `0b001`, `0b010`, `0b100`, `0b101`, `0b110`
    #[error("Invalid byte value")]
    BadByteValue,
}

impl From<DistroRwLockDesErr> for DeserializeError {
    fn from(err: DistroRwLockDesErr) -> Self {
        DeserializeError(anyhow::Error::new(err))
    }
}

#[derive(Debug, Error)]
/// Error when try to acquire or release the lock.
pub enum DistroRwLockError {
    /// the read failed and unable to recover
    #[error("Read failed: {0}")]
    ReadFailed(KvReadError<DistroRwLockValue>),

    /// the update failed
    #[error("Update failed: {0}")]
    UpdateFailed(KvWriteError<DistroRwLockValue>),

    /// thy to release a lock which is already released
    #[error("Already released")]
    AlreadyReleased,

    /// unexpected missing value
    #[error("Unexpected missing value")]
    UnexpectedMissingValue,
}

/// Error when serializing the lock state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("Failed to serialize lock state")]
pub enum DistroRwLockSerErr {}

impl From<DistroRwLockSerErr> for SerializeError {
    fn from(err: DistroRwLockSerErr) -> Self {
        SerializeError(anyhow::Error::new(err))
    }
}


#[derive(Debug, Clone)]
/// A distributed read-write lock.
///
/// 1. It allows multiple read requests of a resource but only one write request.
/// 2. Writing first. If there is a writer waiting, new read request cannot get the lock.
pub struct DistroRwLock {
    key: String,
    value: DistroRwLockValue,
}

impl KeyValue for DistroRwLock {
    type Key = String;
    type Value = DistroRwLockValue;

    fn key(&self) -> Self::Key {
        self.key.clone()
    }

    fn value(&self) -> Self::Value {
        self.value
    }

    fn into_value(self) -> Self::Value {
        self.value
    }

    fn new(key: Self::Key, value: Self::Value) -> Self {
        Self { key, value }
    }
}

impl DistroRwLock {
    /// Try to acquire the read lock.
    pub async fn acquire_read(
        store: &Store,
        key: impl AsRef<str>,
    ) -> Result<(), DistroRwLockError> {
        loop {
            let entry = Self::atomic_read_from(store, key.as_ref().to_string())
                .await
                .map_err(DistroRwLockError::ReadFailed)?;

            if let Some(entry) = entry {
                let lock = entry.value;
                let lock_mode = lock.mode;

                // Check if the lock has expired (held for more than 60s)
                if lock.is_expired() {
                    // If the lock has expired, treat it as Idle and try to acquire it
                    let updated = DistroRwLockValue {
                        mode: DistroRwLockMode::Read,
                        readers: 1,
                        writer_waiting: lock.writer_waiting,
                        acquired_at: DistroRwLockValue::current_timestamp(),
                    };
                    let updated = Self::new(key.as_ref().to_string(), updated);
                    let reversion_result = updated.write_to_atomically(store, entry.revision).await;
                    match reversion_result {
                        Ok(Some(_)) => return Ok(()),
                        Ok(None) => {
                            random_sleep(20, 70).await;
                            continue;
                        }
                        Err(err) => {
                            return Err(DistroRwLockError::UpdateFailed(err));
                        }
                    }
                }

                // block if writer active or waiting.
                // wait the writer to finish.
                if lock_mode == DistroRwLockMode::Write || lock.writer_waiting {
                    random_sleep(50, 120).await;
                    continue;
                }

                // if no block, try to acquire the read lock
                let updated = lock.into_read_acquired();
                let updated = Self::new(key.as_ref().to_string(), updated);
                let reversion_result = updated.write_to_atomically(store, entry.revision).await;
                match reversion_result {
                    // the update succeeded, now we have the lock, and database is updated.
                    Ok(Some(_)) => return Ok(()),

                    // database is uploaded by others, try again.
                    Ok(None) => {
                        random_sleep(20, 70).await;
                        continue;
                    }

                    // connection error or other error than unable to recover
                    Err(err) => {
                        return Err(DistroRwLockError::UpdateFailed(err));
                    }
                }
            } else {
                // create and hold the lock
                let new_lock = Self::new(
                    key.as_ref().to_string(),
                    DistroRwLockValue::new().into_read_acquired(),
                );

                let updated_value = new_lock.create_write(store).await;
                match updated_value {
                    // the update succeeded, now we have the lock, and database is updated.
                    Ok(_) => return Ok(()),

                    // someone else created the lock, try again.
                    Err(KvWriteError::CreateError(create_err))
                        if create_err.kind() == CreateErrorKind::AlreadyExists =>
                    {
                        continue;
                    }

                    // connection error or other error than unable to recover
                    Err(err) => return Err(DistroRwLockError::UpdateFailed(err)),
                }
            }
        }
    }

    /// release the read lock
    pub async fn release_read(
        store: &Store,
        key: impl AsRef<str>,
    ) -> Result<(), DistroRwLockError> {
        loop {
            let entry = Self::atomic_read_from(store, key.as_ref().to_string())
                .await
                .map_err(DistroRwLockError::ReadFailed)?;

            // if we can release a lock, it must be already created
            let Some(entry) = entry else {
                return Err(DistroRwLockError::UnexpectedMissingValue);
            };

            let lock = entry.value;
            if lock.mode != DistroRwLockMode::Read || lock.readers == 0 {
                return Err(DistroRwLockError::AlreadyReleased);
            }

            let updated = lock.into_read_released();
            let updated = Self::new(key.as_ref().to_string(), updated);
            let reversion_result = updated.write_to_atomically(store, entry.revision).await;
            match reversion_result {
                // the update succeeded, now we have the lock, and database is updated.
                Ok(Some(_)) => return Ok(()),

                // database is uploaded by others, try again.
                Ok(None) => {
                    random_sleep(20, 70).await;
                    continue;
                }

                // connection error or other error than unable to recover
                Err(err) => {
                    return Err(DistroRwLockError::UpdateFailed(err));
                }
            }
        }
    }

    /// Set waiter waiting flag.
    ///
    /// If the entry does not exist, it will be created with the flag.
    pub async fn set_writer_waiting(
        store: &Store,
        key: impl AsRef<str>,
        is_waiting: bool,
    ) -> Result<(), DistroRwLockError> {
        loop {
            let entry = Self::atomic_read_from(store, key.as_ref().to_string())
                .await
                .map_err(DistroRwLockError::ReadFailed)?;

            // if the entry exists, update the value
            if let Some(entry) = entry {
                let lock = entry.value;
                if lock.writer_waiting == is_waiting {
                    return Ok(());
                }
            } else {
                // if not, create the entry
                let new_lock = Self::new(
                    key.as_ref().to_string(),
                    DistroRwLockValue::new().into_waiting(is_waiting),
                );
                let updated_value = new_lock.create_write(store).await;
                match updated_value {
                    // the update succeeded, now we have the lock, and database is updated.
                    Ok(_) => return Ok(()),
                    // someone else created the lock, try again.
                    Err(KvWriteError::CreateError(create_err))
                        if create_err.kind() == CreateErrorKind::AlreadyExists =>
                    {
                        continue;
                    }
                    Err(err) => return Err(DistroRwLockError::UpdateFailed(err)),
                }
            };
        }
    }

    /// Try to acquire the write lock.
    pub async fn acquire_write(
        store: &Store,
        key: impl AsRef<str>,
    ) -> Result<(), DistroRwLockError> {
        Self::set_writer_waiting(store, key.as_ref(), true).await?;

        loop {
            let entry = Self::atomic_read_from(store, key.as_ref().to_string())
                .await
                .map_err(DistroRwLockError::ReadFailed)?;

            // we created the entry in `set_writer_waiting`, so if it is None, it is unexpected
            let Some(entry) = entry else {
                return Err(DistroRwLockError::UnexpectedMissingValue);
            };

            let lock = entry.value;

            // Check if the lock has expired (held for more than 60s)
            if lock.is_expired() {
                // If the lock has expired, treat it as Idle and try to acquire it
                let updated = DistroRwLockValue {
                    mode: DistroRwLockMode::Write,
                    readers: 0,
                    writer_waiting: false,
                    acquired_at: DistroRwLockValue::current_timestamp(),
                };
                let updated = Self::new(key.as_ref().to_string(), updated);
                let reversion_result = updated.write_to_atomically(store, entry.revision).await;
                match reversion_result {
                    Ok(Some(_)) => return Ok(()),
                    Ok(None) => {
                        random_sleep(20, 90).await;
                        continue;
                    }
                    Err(err) => {
                        return Err(DistroRwLockError::UpdateFailed(err));
                    }
                }
            }

            // if there is reader, wait
            if lock.mode == DistroRwLockMode::Write || lock.readers > 0 {
                random_sleep(50, 120).await;
                continue;
            }

            let updated = lock.into_write_acquired();
            let updated = Self::new(key.as_ref().to_string(), updated);
            let reversion_result = updated.write_to_atomically(store, entry.revision).await;
            match reversion_result {
                // the update succeeded, now we have the lock, and database is updated.
                Ok(Some(_)) => return Ok(()),

                // database is uploaded by others, try again.
                Ok(None) => {
                    random_sleep(20, 90).await;
                    continue;
                }

                // connection error or other error than unable to recover
                Err(err) => {
                    return Err(DistroRwLockError::UpdateFailed(err));
                }
            }
        }
    }

    /// Release the write lock.
    pub async fn release_write(
        store: &Store,
        key: impl AsRef<str>,
    ) -> Result<(), DistroRwLockError> {
        loop {
            let entry = Self::atomic_read_from(store, key.as_ref().to_string())
                .await
                .map_err(DistroRwLockError::ReadFailed)?;

            // if we can release a lock, it must be already created
            let Some(entry) = entry else {
                return Err(DistroRwLockError::UnexpectedMissingValue);
            };

            let lock = entry.value;
            if lock.mode != DistroRwLockMode::Write {
                return Err(DistroRwLockError::AlreadyReleased);
            }

            let updated = lock.into_write_released();
            let updated = Self::new(key.as_ref().to_string(), updated);
            let reversion_result = updated.write_to_atomically(store, entry.revision).await;
            match reversion_result {
                // the update succeeded, now we have the lock, and database is updated.
                Ok(Some(_)) => return Ok(()),

                // database is uploaded by others, try again.
                Ok(None) => {
                    random_sleep(20, 50).await;
                    continue;
                }

                // connection error or other error than unable to recover
                Err(err) => {
                    return Err(DistroRwLockError::UpdateFailed(err));
                }
            }
        }
    }
}

#[inline]
/// Random sleep for a duration between `min_ms` and `max_ms`.
async fn random_sleep(min_ms: u64, max_ms: u64) {
    let sleep_time = rand::rng().random_range(min_ms..max_ms);
    tokio::time::sleep(Duration::from_millis(sleep_time)).await;
}

#[derive(Debug, Error)]
/// The error result of [LockedResourceReadProcessor] and [LockedResourceWriteProcessor]
pub enum WithLockProcessError {
    /// The error when try to acquire or release the lock.
    #[error("Failed to acquire lock: {0}")]
    FailOnAcquire(DistroRwLockError),

    /// The error when try to acquire or release the lock.
    #[error("Failed to release lock: {0}")]
    FailOnRelease(DistroRwLockError),
}

/// A RAII wrapper around a [Processor] that acquires a lock to read the resource before processing and releases it after.
pub struct LockedResourceReadProcessor<I, O, P, K> 
where 
    P: Processor<I, O> + Send + Sync,
    K: AsRef<str> + Send + Sync,
    I: Send + Sync,
    O: Send + Sync,
{
    processor: Arc<P>,
    store: &'static Store,
    key: K,
    phantom_input: PhantomData<I>,
    phantom_output: PhantomData<O>,
}

impl<I,O,P,K> Processor<I, Result<O, WithLockProcessError>> for LockedResourceReadProcessor<I, O, P, K>
where 
    P: Processor<I, O> + Send + Sync,
    K: AsRef<str> + Send + Sync,
    I: Send + Sync,
    O: Send + Sync,
{
    async fn process(&self, input: I) -> Result<O, WithLockProcessError> {
        DistroRwLock::acquire_read(self.store, self.key.as_ref())
            .await
            .map_err(WithLockProcessError::FailOnAcquire)?;
        let result = self.processor.process(input).await;
        DistroRwLock::release_read(self.store, self.key.as_ref())
            .await
            .map_err(WithLockProcessError::FailOnRelease)?;
        Ok(result)
    }
}

impl<I,O,P,K> LockedResourceReadProcessor<I,O,P,K>
where 
    P: Processor<I, O> + Send + Sync,
    K: AsRef<str> + Send + Sync,
    I: Send + Sync,
    O: Send + Sync,
{
    /// Create a new [LockedResourceReadProcessor]
    pub fn new(processor: Arc<P>, store: &'static Store, key: K) -> Self {
        Self {
            processor,
            store,
            key,
            phantom_input: PhantomData,
            phantom_output: PhantomData,
        }
    }
}

/// A RAII wrapper around a [Processor] that acquires a lock to write the resource before processing and releases it after.
pub struct LockedResourceWriteProcessor<I, O, P, K> 
where 
    P: Processor<I, O> + Send + Sync,
    K: AsRef<str> + Send + Sync,
    I: Send + Sync,
    O: Send + Sync,
{
    processor: Arc<P>,
    store: &'static Store,
    key: K,
    phantom_input: PhantomData<I>,
    phantom_output: PhantomData<O>,
}

impl<I,O,P,K> Processor<I, Result<O, WithLockProcessError>> for LockedResourceWriteProcessor<I, O, P, K>
where 
    P: Processor<I, O> + Send + Sync,
    K: AsRef<str> + Send + Sync,
    I: Send + Sync,
    O: Send + Sync,
{
    async fn process(&self, input: I) -> Result<O, WithLockProcessError> {
        DistroRwLock::acquire_write(self.store, self.key.as_ref())
            .await
            .map_err(WithLockProcessError::FailOnAcquire)?;

        let result = self.processor.process(input).await;

        DistroRwLock::release_write(self.store, self.key.as_ref())
            .await
            .map_err(WithLockProcessError::FailOnRelease)?;

        Ok(result)
    }
}

impl<I,O,P,K> LockedResourceWriteProcessor<I,O,P,K>
where 
    P: Processor<I, O> + Send + Sync,
    K: AsRef<str> + Send + Sync,
    I: Send + Sync,
    O: Send + Sync,
{
    /// Create a new [LockedResourceWriteProcessor]
    pub fn new(processor: Arc<P>, store: &'static Store, key: K) -> Self {
        Self {
            processor,
            store,
            key,
            phantom_input: PhantomData,
            phantom_output: PhantomData,
        }
    }
}
