use serde::de::Error;

pub mod rpc;
pub mod simple_push;

pub trait NatsJsonMessage
where Self: serde::Serialize + for <'de> serde::de::Deserialize<'de> + Sized
{
    fn subject() -> &'static str;
    fn to_json_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        let json = serde_json::to_string(self)?;
        Ok(json.into_bytes())
    }
    fn from_json_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        let json = std::str::from_utf8(bytes).map_err(|_| serde_json::Error::custom("Failed to parse string from bytes"))?;
        let rpc_request = serde_json::from_str(json)?;
        Ok(rpc_request)
    }
}

