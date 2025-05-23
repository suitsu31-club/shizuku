use crate::error::{DeserializeError, SerializeError};
use crate::kv::{KeyValue, KeyValueRead, KeyValueWrite, KvReadError, KvWriteError};
use crate::{ByteDeserialize, ByteSerialize};
use async_nats::jetstream::kv::{CreateErrorKind, Store};
use rand::Rng;
use std::fmt::{Display, Formatter};
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;
use kanau::processor::Processor;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// The Kernel of [DistroRwLock]
pub struct DistroRwLockValue {
    /// The current mode of the lock.
    pub mode: DistroRwLockMode,

    /// The number of readers.
    pub readers: u64,

    /// Whether there is a writer waiting.
    pub writer_waiting: bool,
}

impl DistroRwLockValue {
    /// Create a new lock.
    pub fn new() -> Self {
        Self::default()
    }

    /// Update the lock into a read acquired state.
    #[inline]
    pub fn into_read_acquired(self) -> Self {
        Self {
            mode: DistroRwLockMode::Read,
            readers: self.readers + 1,
            writer_waiting: self.writer_waiting,
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
        }
    }

    /// Update whether there is a writer waiting.
    #[inline]
    pub fn into_waiting(self, writer_waiting: bool) -> Self {
        Self {
            mode: self.mode,
            readers: self.readers,
            writer_waiting,
        }
    }

    /// Update the lock into a write acquired state.
    #[inline]
    pub fn into_write_acquired(self) -> Self {
        Self {
            mode: DistroRwLockMode::Write,
            readers: 0,
            writer_waiting: false,
        }
    }

    /// Update the lock into a write released state.
    pub fn into_write_released(self) -> Self {
        Self {
            mode: DistroRwLockMode::Idle,
            readers: 0,
            writer_waiting: self.writer_waiting,
        }
    }
}

/// The current mode of the lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DistroRwLockMode {
    /// the lock is held by a reader
    Read,

    /// the lock is held by a writer
    Write,

    #[default]
    /// the lock is not held
    Idle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Error when deserializing the lock state.
pub enum DistroRwLockDesErr {
    /// the length of the bytes is not 9
    InvalidByteLength,

    /// the first byte is not `0b000`, `0b001`, `0b010`, `0b100`, `0b101`, `0b110`
    BadByteValue,
}

impl Display for DistroRwLockDesErr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DistroRwLockDesErr::InvalidByteLength => write!(f, "Invalid byte length"),
            DistroRwLockDesErr::BadByteValue => write!(f, "Bad byte value"),
        }
    }
}

impl std::error::Error for DistroRwLockDesErr {}

impl From<DistroRwLockDesErr> for DeserializeError {
    fn from(err: DistroRwLockDesErr) -> Self {
        DeserializeError(anyhow::Error::new(err))
    }
}

#[derive(Debug)]
/// Error when try to acquire or release the lock.
pub enum DistroRwLockError {
    /// the read failed and unable to recover
    ReadFailed(KvReadError<DistroRwLockValue>),

    /// the update failed
    UpdateFailed(KvWriteError<DistroRwLockValue>),

    /// thy to release a lock which is already released
    AlreadyReleased,

    /// unexpected missing value
    UnexpectedMissingValue,
}

impl Display for DistroRwLockError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DistroRwLockError::ReadFailed(err) => write!(f, "Read failed: {}", err),
            DistroRwLockError::UpdateFailed(err) => write!(f, "Update failed: {}", err),
            DistroRwLockError::AlreadyReleased => write!(f, "Already released"),
            DistroRwLockError::UnexpectedMissingValue => write!(f, "Unexpected missing value"),
        }
    }
}

impl std::error::Error for DistroRwLockError {}

/// Error when serializing the lock state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistroRwLockSerErr {}

impl Display for DistroRwLockSerErr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to serialize lock state")
    }
}

impl std::error::Error for DistroRwLockSerErr {}

impl From<DistroRwLockSerErr> for SerializeError {
    fn from(err: DistroRwLockSerErr) -> Self {
        SerializeError(anyhow::Error::new(err))
    }
}

impl ByteSerialize for DistroRwLockValue {
    type SerError = DistroRwLockSerErr;

    fn to_bytes(&self) -> Result<Box<[u8]>, Self::SerError> {
        let mut bytes: [u8; 9] = [0b00000000; 9];
        match (&self.writer_waiting, &self.mode) {
            (false, DistroRwLockMode::Idle) => bytes[0] = 0b000,
            (false, DistroRwLockMode::Read) => bytes[0] = 0b001,
            (false, DistroRwLockMode::Write) => bytes[0] = 0b010,
            (true, DistroRwLockMode::Idle) => bytes[0] = 0b100,
            (true, DistroRwLockMode::Read) => bytes[0] = 0b101,
            (true, DistroRwLockMode::Write) => bytes[0] = 0b110,
        }
        bytes[1..9].copy_from_slice(&self.readers.to_be_bytes());
        Ok(bytes.into())
    }
}

impl ByteDeserialize for DistroRwLockValue {
    type DeError = DistroRwLockDesErr;

    fn parse_from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self, Self::DeError> {
        let bytes = bytes.as_ref();
        if bytes.len() != 9 {
            return Err(DistroRwLockDesErr::InvalidByteLength);
        }

        let state = bytes[0];
        let (writer_waiting, mode) = match state {
            0b000 => (false, DistroRwLockMode::Idle),
            0b001 => (false, DistroRwLockMode::Read),
            0b010 => (false, DistroRwLockMode::Write),
            0b100 => (true, DistroRwLockMode::Idle),
            0b101 => (true, DistroRwLockMode::Read),
            0b110 => (true, DistroRwLockMode::Write),
            _ => return Err(DistroRwLockDesErr::BadByteValue),
        };

        let readers = u64::from_be_bytes(bytes[1..9].try_into().map_err(|_| {
            DistroRwLockDesErr::InvalidByteLength
        })?);

        Ok(Self {
            mode,
            readers,
            writer_waiting,
        })
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

#[derive(Debug)]
/// The error result of [LockedResourceReadProcessor] and [LockedResourceWriteProcessor]
pub enum WithLockProcessError {
    /// The error when try to acquire or release the lock.
    FailOnAcquire(DistroRwLockError),

    /// The error when try to acquire or release the lock.
    FailOnRelease(DistroRwLockError),
}

impl Display for WithLockProcessError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FailOnAcquire(err) => write!(f, "Failed to acquire lock: {}", err),
            Self::FailOnRelease(err) => write!(f, "Failed to release lock: {}", err),
        }
    }
}

impl std::error::Error for WithLockProcessError {}

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

