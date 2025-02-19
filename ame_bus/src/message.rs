use std::sync::Arc;
use serde::ser::Error;

pub trait NatsMessage
where
    Self: Sized + Send,
{
    type SerError: Error;
    type DeError: Error;

    fn to_bytes(&self) -> Result<Arc<[u8]>, Self::SerError>;

    fn from_bytes(bytes: &[u8]) -> Result<Self, Self::DeError>;
}

pub trait NatsJsonMessage
where
    Self: serde::Serialize + for<'de> serde::de::Deserialize<'de> + Sized + Send,
{
    fn to_json_bytes(&self) -> Result<Arc<[u8]>, serde_json::Error> {
        let json = serde_json::to_string(self)?;
        Ok(Arc::from(json.into_bytes()))
    }
    fn from_json_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        let json = std::str::from_utf8(bytes)
            .map_err(|_| serde_json::Error::custom("Failed to parse string from bytes"))?;
        let rpc_request = serde_json::from_str(json)?;
        Ok(rpc_request)
    }
}

impl NatsJsonMessage for () {}
impl NatsJsonMessage for serde_json::Value {}

impl<T: NatsJsonMessage> NatsMessage for T {
    type SerError = serde_json::Error;
    type DeError = serde_json::Error;

    fn to_bytes(&self) -> Result<Arc<[u8]>, Self::SerError> {
        self.to_json_bytes()
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, Self::DeError> {
        Self::from_json_bytes(bytes)
    }
}