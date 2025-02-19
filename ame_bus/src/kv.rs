use crate::NatsMessage;
use async_nats::jetstream::kv;
use std::marker::PhantomData;

#[async_trait::async_trait]
pub trait NatsKvValue: NatsMessage {
    type Key: Into<String>;
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
    ) -> anyhow::Result<Option<NatsKvEntry<Self::Key, Self>>>
    where
        Self: Sized,
    {
        let key = key.into();
        let entry = store.entry(key).await?;
        let entry = entry.map(NatsKvEntry::new);
        Ok(entry)
    }
    async fn get(store: &kv::Store, key: Self::Key) -> anyhow::Result<Option<Self>>
    where
        Self: Sized,
    {
        Ok(store
            .get(key.into())
            .await?
            .map(|value| Self::parse_from_bytes(&value)?))
    }
}

pub struct NatsKv<K: Into<String>, V: NatsKvValue<Key = K>>(pub K, pub V);

impl<K, V> NatsKv<K, V> {
    pub fn new(key: K, value: V) -> Self {
        Self(key, value)
    }
    pub async fn put(self, store: &kv::Store) -> anyhow::Result<()> {
        let key = self.0.into();
        let value = self.1.to_bytes()?;
        store.put(key, value.into()).await?;
        Ok(())
    }
}

pub struct NatsKvEntry<K: Into<String>, V: NatsKvValue<Key = K>> {
    pub entry: kv::Entry,
    value: PhantomData<V>,
}

impl<K, V> NatsKvEntry<K, V> {
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
