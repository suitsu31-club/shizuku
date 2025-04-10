/// integrated distro rw lock
pub mod rw_lock;

use crate::kv::kv::Watch;
use crate::kv::kv::WatchError;
use crate::{ByteDeserialize, ByteSerialize};
use async_nats::jetstream::kv;
use async_nats::jetstream::kv::{
    CreateError, Entry, EntryError, PutError, Store, UpdateError, UpdateErrorKind,
};
use std::fmt::{Debug, Display, Formatter};

// ---------------------------------------------

/// A value in KV store that has a static key.
pub trait StaticKeyIndexedValue {
    /// The key of the value. Must be constant.
    fn key() -> String;
}

impl<T: StaticKeyIndexedValue + Clone + Send + Sync> KeyValue for T {
    type Key = String;
    type Value = T;

    fn key(&self) -> Self::Key {
        Self::key()
    }

    fn value(&self) -> Self::Value {
        self.clone()
    }

    fn into_value(self) -> Self::Value {
        self
    }

    fn new(_key: Self::Key, value: Self::Value) -> Self {
        value
    }
}

/// A struct that have both the key and value.
///
/// If a value has a static key (by implementing [StaticKeyIndexedValue]),
/// and it implements `Clone`, it can implement this trait automatically.
pub trait KeyValue: Sized + Send + Sync {
    /// The key type.
    type Key: Into<String> + Send + Sync + Sized;

    /// The value type.
    type Value: Send + Sync + Sized;

    /// The key of the value. Must be dynamic.
    fn key(&self) -> Self::Key;

    /// Get the value.
    fn value(&self) -> Self::Value;

    /// Get the value by moving the value out.
    fn into_value(self) -> Self::Value;

    /// Create the pair from key and value.
    fn new(key: Self::Key, value: Self::Value) -> Self;

    /// Delete the value from the store.
    fn delete_anyway(
        store: &Store,
        key: Self::Key,
    ) -> impl Future<Output = Result<(), async_nats::Error>> + Send {
        async move {
            let key: String = key.into();
            store.delete(key).await?;
            Ok(())
        }
    }

    /// Delete the value from the store atomically.
    ///
    /// The `revision` is the expected revision of the value. If the revision is not matched, the delete will fail.
    fn delete_atomically(
        store: &Store,
        key: Self::Key,
        revision: u64,
    ) -> impl Future<Output = Result<(), async_nats::Error>> + Send {
        async move {
            let key: String = key.into();
            store.delete_expect_revision(key, Some(revision)).await?;
            Ok(())
        }
    }

    /// Purges all the revisions of an entry destructively, leaving behind a single purge entry in-place.
    fn purge(
        store: &Store,
        key: Self::Key,
    ) -> impl Future<Output = Result<(), async_nats::Error>> + Send {
        async move {
            let key: String = key.into();
            store.purge(key).await?;
            Ok(())
        }
    }
}

// ---------------------------------------------

/// Result from atomic read or history read
pub struct KvEntry<V: ByteDeserialize> {
    /// Name of the bucket the entry is in.
    pub bucket: String,

    /// The key that was retrieved.
    pub key: String,

    /// The value that was retrieved.
    pub value: V,

    /// A unique sequence for this value.
    pub revision: u64,

    /// Distance from the latest value.
    pub delta: u64,

    /// The time the data was put in the bucket.
    pub created_at: time::OffsetDateTime,

    /// The kind of operation that caused this entry.
    pub operation: kv::Operation,

    /// Set to true after all historical messages have been received, and now all Entries are the new ones.
    pub seen_current: bool,
}

impl<V: ByteDeserialize> TryFrom<Entry> for KvEntry<V> {
    type Error = V::DeError;

    fn try_from(value: Entry) -> Result<Self, Self::Error> {
        Ok(Self {
            bucket: value.bucket,
            key: value.key,
            value: V::parse_from_bytes(value.value)?,
            revision: value.revision,
            delta: value.delta,
            created_at: value.created,
            operation: value.operation,
            seen_current: value.seen_current,
        })
    }
}

// ---------------------------------------------

#[derive(Debug)]
/// Error when reading from KV store.
pub enum KvReadError<V: ByteDeserialize> {
    /// Error when reading from KV store.
    EntryError(EntryError),
    /// Error when deserializing the value.
    DeserializeError(V::DeError),
}

impl<V: ByteDeserialize> From<EntryError> for KvReadError<V> {
    fn from(err: EntryError) -> Self {
        Self::EntryError(err)
    }
}

impl<V: ByteDeserialize> Display for KvReadError<V> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EntryError(err) => write!(f, "Entry error: {}", err),
            Self::DeserializeError(err) => write!(f, "Deserialize error: {}", err),
        }
    }
}

impl<V: ByteDeserialize + Debug> std::error::Error for KvReadError<V> {}

/// A value that can be read from KV store.
///
/// If the `Value` type implements [ByteDeserialize], and
/// the `Key` type implements `Clone`, it can implement this trait automatically.
pub trait KeyValueRead: KeyValue
where
    Self::Value: ByteDeserialize,
    Self::Key: Clone,
{
    /// Read the value from the store.
    fn read_from(
        store: &Store,
        key: Self::Key,
    ) -> impl Future<Output = Result<Option<Self>, KvReadError<Self::Value>>> + Send {
        async move {
            store
                .get(key.clone())
                .await?
                .map(|value| {
                    Self::Value::parse_from_bytes(value)
                        .map_err(KvReadError::DeserializeError)
                        .map(|parsed| Self::new(key.clone(), parsed))
                })
                .transpose()
        }
    }

    /// Read the value from the store atomically.
    ///
    /// Always get the latest version
    fn atomic_read_from(
        store: &Store,
        key: Self::Key,
    ) -> impl Future<Output = Result<Option<KvEntry<Self::Value>>, KvReadError<Self::Value>>> + Send
    {
        async move {
            store
                .entry(key.clone())
                .await?
                .map(|value| KvEntry::try_from(value).map_err(KvReadError::DeserializeError))
                .transpose()
        }
    }

    /// Read a history version of the value from the store.
    fn history_read_from(
        store: &Store,
        key: Self::Key,
        revision: u64,
    ) -> impl Future<Output = Result<Option<KvEntry<Self::Value>>, KvReadError<Self::Value>>> + Send
    {
        async move {
            store
                .entry_for_revision(key.clone(), revision)
                .await?
                .map(|entry| KvEntry::try_from(entry).map_err(KvReadError::DeserializeError))
                .transpose()
        }
    }

    /// Watch the value in the store. Yields the value when it is updated.
    fn watch(
        store: &Store,
        key: Self::Key,
    ) -> impl Future<Output = Result<Watch, WatchError>> + Send {
        async move {
            let key: String = key.into();
            store.watch(key).await
        }
    }
}

impl<T: KeyValue> KeyValueRead for T
where
    T::Value: ByteDeserialize,
    T::Key: Clone,
{
}

// ---------------------------------------------

#[derive(Debug)]
/// Error when writing to KV store.
pub enum KvWriteError<V: ByteSerialize> {
    /// Error when writing to KV store atomically.
    UpdateError(UpdateError),

    /// Error when writing to KV store.
    PutError(PutError),

    /// Error when serializing the value.
    SerializeError(V::SerError),

    /// Error when creating the value.
    CreateError(CreateError),
}

impl<V: ByteSerialize> From<UpdateError> for KvWriteError<V> {
    fn from(err: UpdateError) -> Self {
        Self::UpdateError(err)
    }
}

impl<V: ByteSerialize> From<PutError> for KvWriteError<V> {
    fn from(err: PutError) -> Self {
        Self::PutError(err)
    }
}

impl<V: ByteSerialize> Display for KvWriteError<V> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UpdateError(err) => write!(f, "Update error: {}", err),
            Self::PutError(err) => write!(f, "Put error: {}", err),
            Self::SerializeError(err) => write!(f, "Serialize error: {}", err),
            Self::CreateError(err) => write!(f, "Create error: {}", err),
        }
    }
}

impl<V: ByteSerialize + Debug> std::error::Error for KvWriteError<V> {}

/// A value that can be written to KV store.
pub trait KeyValueWrite: KeyValue
where
    Self::Value: ByteSerialize,
    Self::Key: Into<String> + Send + Sync + Sized,
{
    /// Write the value to the store.
    fn write_to_anyway(
        &self,
        store: &Store,
    ) -> impl Future<Output = Result<(), KvWriteError<Self::Value>>> + Send {
        async move {
            let key: String = self.key().into();
            let bytes = self
                .value()
                .to_bytes()
                .map_err(KvWriteError::SerializeError)?;
            store
                .put(key, bytes.into())
                .await
                .map_err(KvWriteError::PutError)?;
            Ok(())
        }
    }

    /// Write the value to the store atomically.
    ///
    /// The `revision` is the expected revision of the value. If the revision is not matched, the write will fail and return `Ok(None)`.
    fn write_to_atomically(
        &self,
        store: &Store,
        revision: u64,
    ) -> impl Future<Output = Result<Option<u64>, KvWriteError<Self::Value>>> + Send {
        async move {
            let key: String = self.key().into();
            let bytes = self
                .value()
                .to_bytes()
                .map_err(KvWriteError::SerializeError)?;
            let new_version = store.update(key, bytes.into(), revision).await;
            match new_version {
                Ok(new_version) => Ok(Some(new_version)),
                Err(err) if err.kind() == UpdateErrorKind::WrongLastRevision => Ok(None),
                Err(err) => Err(KvWriteError::UpdateError(err)),
            }
        }
    }

    /// Create the value in the store. Failed if the value already exists.
    fn create_write(
        &self,
        store: &Store,
    ) -> impl Future<Output = Result<(), KvWriteError<Self::Value>>> + Send {
        async move {
            let key: String = self.key().into();
            let bytes = self
                .value()
                .to_bytes()
                .map_err(KvWriteError::SerializeError)?;
            store
                .create(key, bytes.into())
                .await
                .map_err(KvWriteError::CreateError)?;
            Ok(())
        }
    }
}

impl<T: KeyValue> KeyValueWrite for T
where
    T::Value: ByteSerialize,
    T::Key: Into<String> + Send + Sync + Sized,
{
}
