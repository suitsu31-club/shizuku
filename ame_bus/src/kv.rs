use crate::NatsMessage;
use async_nats::jetstream::kv;
use std::marker::PhantomData;

#[async_trait::async_trait]
/// A Value that can be stored in the NATS JetStream Key/Value Store.
pub trait NatsKvValue: NatsMessage + Sync + Send {
    /// The key type for the value. Usually `String` or `&'static str`.
    ///
    /// The key type must implement `Into<String>` if the key is dynamic.
    /// If the Key is static, you can use `&'static str` as the Key type.
    type Key: Into<String> + Send + Sync;

    /// The name of the bucket
    const BUCKET: &'static str;

    /// The configuration for the store.
    fn store_config() -> kv::Config {
        kv::Config {
            bucket: Self::BUCKET.to_owned(),
            ..Default::default()
        }
    }

    /// Attach `Self::Key`, combine the key with the value into [NatsKv]
    fn with_key(self, key: Self::Key) -> NatsKv<Self::Key, Self> {
        NatsKv::new(key, self)
    }

    /// Retrieves the last [Entry](NatsKvEntry) for a given key from a bucket.
    async fn entry(
        store: &kv::Store,
        key: Self::Key,
    ) -> anyhow::Result<Option<NatsKvEntry<Self::Key, Self>>> {
        let key = key.into();
        let entry = store.entry(key).await?;
        let entry = entry.map(NatsKvEntry::new);
        Ok(entry)
    }

    /// Retrieves the value for a given key from a bucket.
    async fn get(store: &kv::Store, key: Self::Key) -> anyhow::Result<Option<Self>> {
        let value = store
            .get(key.into())
            .await?
            .map(|value| Self::parse_from_bytes(&value));
        Ok(value.transpose()?)
    }

    /// Mark a value as deleted by key.
    async fn delete_by_key(store: &kv::Store, key: Self::Key) -> anyhow::Result<()> {
        store.delete(key.into()).await?;
        Ok(())
    }

    /// Update a value as deleted by key,
    /// if the version in this function is the latest version in the store.
    async fn delete_expect_revision(
        store: &kv::Store,
        key: Self::Key,
        version: u64,
    ) -> anyhow::Result<()> {
        store
            .delete_expect_revision(key.into(), Some(version))
            .await?;
        Ok(())
    }
}

/// A Key/Value pair for the NATS JetStream Key/Value Store.
///
/// The key must be static.
pub trait NatsStaticKeyKvValue: NatsMessage + Sync + Send {
    /// The key of kv pair
    const KEY: &'static str;

    /// The name of the bucket
    const BUCKET: &'static str;
}

impl<T> NatsKvValue for T
    where T: NatsStaticKeyKvValue
{
    type Key = &'static str;
    const BUCKET: &'static str = <Self as NatsStaticKeyKvValue>::BUCKET;
}

/// A Key/Value pair for the NATS JetStream Key/Value Store.
pub struct NatsKv<K, V>(pub K, pub V)
where
    K: Into<String> + Send + Sync,
    V: NatsKvValue<Key = K> + Send + Sync;

impl<K, V> NatsKv<K, V>
where
    K: Into<String> + Send + Sync,
    V: NatsKvValue<Key = K> + Send + Sync,
{
    /// Create a new Key/Value pair.
    pub fn new(key: K, value: V) -> Self {
        Self(key, value)
    }

    /// Put the value into the store.
    ///
    /// If the key already exists, it will be overwritten.
    pub async fn put(self, store: &kv::Store) -> anyhow::Result<()> {
        let key = self.0.into();
        let value = self.1.to_bytes()?;
        store.put(key, value.to_vec().into()).await?;
        Ok(())
    }

    /// Update the value in the store.
    /// If the version in this function is the latest version in the store.
    ///
    /// This is very useful for atomic operations and synchronization.
    pub async fn update(self, store: &kv::Store, reversion: u64) -> anyhow::Result<u64> {
        let key = self.0.into();
        let value = self.1.to_bytes()?;
        let new = store.update(key, value.to_vec().into(), reversion).await?;
        Ok(new)
    }
}

/// A wrapper around a [NATS SDK's Entry](kv::Entry).
pub struct NatsKvEntry<K: Into<String>, V: NatsKvValue<Key = K>> {
    #[allow(missing_docs)]
    pub entry: kv::Entry,
    value: PhantomData<V>,
}

impl<K, V> NatsKvEntry<K, V>
where
    K: Into<String> + Send + Sync,
    V: NatsKvValue<Key = K> + Send + Sync,
{
    #[allow(missing_docs)]
    pub fn new(entry: kv::Entry) -> Self {
        Self {
            entry,
            value: PhantomData,
        }
    }

    /// Parse the value from the entry.
    pub fn value(&self) -> anyhow::Result<V> {
        let value = V::parse_from_bytes(&self.entry.value)?;
        Ok(value)
    }
}
