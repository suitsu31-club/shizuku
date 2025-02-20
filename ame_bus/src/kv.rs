use crate::NatsMessage;
use async_nats::jetstream::kv;
use std::marker::PhantomData;

#[async_trait::async_trait]
pub trait NatsKvValue: NatsMessage + Sync + Send {
    type Key: Into<String> + Send + Sync;
    const BUCKET: &'static str;
    fn store_config() -> kv::Config {
        kv::Config {
            bucket: Self::BUCKET.to_owned(),
            ..Default::default()
        }
    }
    fn with_key(self, key: Self::Key) -> NatsKv<Self::Key, Self> {
        NatsKv::new(key, self)
    }

    async fn entry(
        store: &kv::Store,
        key: Self::Key,
    ) -> anyhow::Result<Option<NatsKvEntry<Self::Key, Self>>> {
        let key = key.into();
        let entry = store.entry(key).await?;
        let entry = entry.map(NatsKvEntry::new);
        Ok(entry)
    }

    async fn get(store: &kv::Store, key: Self::Key) -> anyhow::Result<Option<Self>> {
        let value = store
            .get(key.into())
            .await?
            .map(|value| Self::parse_from_bytes(&value));
        Ok(value.transpose()?)
    }

    async fn delete_by_key(store: &kv::Store, key: Self::Key) -> anyhow::Result<()> {
        store.delete(key.into()).await?;
        Ok(())
    }

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

pub struct NatsKv<K, V>(pub K, pub V)
where
    K: Into<String> + Send + Sync,
    V: NatsKvValue<Key = K> + Send + Sync;

impl<K, V> NatsKv<K, V>
where
    K: Into<String> + Send + Sync,
    V: NatsKvValue<Key = K> + Send + Sync,
{
    pub fn new(key: K, value: V) -> Self {
        Self(key, value)
    }
    pub async fn put(self, store: &kv::Store) -> anyhow::Result<()> {
        let key = self.0.into();
        let value = self.1.to_bytes()?;
        store.put(key, value.to_vec().into()).await?;
        Ok(())
    }
    pub async fn update(self, store: &kv::Store, reversion: u64) -> anyhow::Result<u64> {
        let key = self.0.into();
        let value = self.1.to_bytes()?;
        let new = store.update(key, value.to_vec().into(), reversion).await?;
        Ok(new)
    }
}

pub struct NatsKvEntry<K: Into<String>, V: NatsKvValue<Key = K>> {
    pub entry: kv::Entry,
    value: PhantomData<V>,
}

impl<K, V> NatsKvEntry<K, V>
where
    K: Into<String> + Send + Sync,
    V: NatsKvValue<Key = K> + Send + Sync,
{
    pub fn new(entry: kv::Entry) -> Self {
        Self {
            entry,
            value: PhantomData,
        }
    }
    pub fn value(&self) -> anyhow::Result<V> {
        let value = V::parse_from_bytes(&self.entry.value)?;
        Ok(value)
    }
}
