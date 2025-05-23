use async_nats::jetstream::kv::Store;
use kanau::processor::Processor;
use thiserror::Error;
use crate::{kv, ByteDeserialize, ByteSerialize};
use crate::kv::{KeyValueRead, KeyValueWrite, KvEntry, KvReadError, KvWriteError};

#[derive(Debug)]
/// A processor that can do atomic operates in NATS KV Store
pub struct AtomicOptProcessor<'a> {
    store: &'a Store
}

impl<'a> AtomicOptProcessor<'a> {
    /// capture a reference of store
    pub fn new(store: &'a Store) -> Self {
        AtomicOptProcessor { store }
    }
}

#[derive(Debug, Clone)]
/// A map command that convert [KvEntry] to `KV::Value`
pub struct AtomicMap<
    KV: KeyValueRead + KeyValueWrite,
    P: Processor<KvEntry<KV::Value>, KV::Value>
> 
where 
    <KV as kv::KeyValue>::Key: Clone,
    <KV as kv::KeyValue>::Value: ByteDeserialize + ByteSerialize,
{
    /// The key used to find the target key-value pair
    pub key: KV::Key,
    
    /// The operate function (processor)
    pub opt: P,
    
    /// The retry times. If `None`, will retry until success
    pub retry_time: Option<u64>,
}

#[derive(Debug, Error)]
/// Error in atomic operation
pub enum AtomicOptError<T: ByteDeserialize + ByteSerialize> {
    #[error("KvStore error when read: {0}")]
    /// KvStore error when read
    ReadError(KvReadError<T>),
    
    #[error("Try to operate on an item that doesn't exist")]
    /// Try to operate on an item that doesn't exist
    NotFound,
    
    #[error("The `retry_time` value is 0")]
    /// The `retry_time` value is 0
    NeverTry,
    
    #[error("KvStore error when writing: {0}")]
    /// KvStore error when writing
    WriteError(KvWriteError<T>),
}

/// The result of [AtomicMap]
pub type AtomicMapResult<V> = Result<(), AtomicOptError<V>>;

impl<KV, P> Processor<AtomicMap<KV, P>, AtomicMapResult<KV::Value>> for AtomicOptProcessor<'_>
where 
    KV: KeyValueRead + KeyValueWrite,
    P: Processor<KvEntry<KV::Value>, KV::Value> + Send,
    KV::Key: Clone,
    KV::Value: ByteDeserialize + ByteSerialize,
{
    async fn process(&self, input: AtomicMap<KV, P>) -> AtomicMapResult<KV::Value> {
        if let Some(retry_time) = input.retry_time {
            let mut result = AtomicOptError::NeverTry;
            for _ in 0..retry_time {
                let entry_result = match KV::atomic_read_from(
                    self.store, input.key.clone()
                )
                    .await {
                    Ok(Some(entry)) => entry,
                    Ok(None) => return Err(AtomicOptError::NotFound),
                    Err(e) => {
                        result = AtomicOptError::ReadError(e);
                        continue
                    }
                };
                let reversion = entry_result.revision;
                let converted = input.opt.process(entry_result).await;
                let converted_kv = KV::new(input.key.clone(), converted);
                match converted_kv.write_to_atomically(
                    self.store,
                    reversion
                ).await {
                    Ok(_) => return Ok(()),
                    Err(e) => {
                        result = AtomicOptError::WriteError(e);
                    }
                }
            }
            Err(result)
        } else {
            loop {
                let entry_result = match KV::atomic_read_from(
                    self.store, input.key.clone()
                ).await {
                    Ok(Some(entry)) => entry,
                    Ok(None) => return Err(AtomicOptError::NotFound),
                    Err(_) => continue,
                };
                
                let reversion = entry_result.revision;
                let converted = input.opt.process(entry_result).await;
                let converted_kv = KV::new(input.key.clone(), converted);
                
                if converted_kv.write_to_atomically(
                    self.store, reversion
                ).await.is_ok() {
                    return Ok(());
                }
            }
        }
    }
}