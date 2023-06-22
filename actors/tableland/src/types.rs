use fvm_ipld_encoding::strict_bytes;
use fvm_ipld_encoding::tuple::*;
use rusqlite::types::Value;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_with::{serde_as, DeserializeAs, SerializeAs};

#[derive(Debug, Serialize_tuple, Deserialize_tuple)]
#[serde(transparent)]
pub struct ConstructorParams {
    #[serde(with = "strict_bytes")]
    pub db: Vec<u8>,
}

#[derive(Debug, Serialize_tuple, Deserialize_tuple)]
#[serde(transparent)]
pub struct QueryParams {
    pub stmt: String,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct QueryReturn {
    pub cols: Vec<String>,
    #[serde_as(as = "Vec<Vec<ValueDef>>")]
    pub rows: Vec<Vec<Value>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(remote = "Value", untagged)]
pub enum ValueDef {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl SerializeAs<Value> for ValueDef {
    fn serialize_as<S>(value: &Value, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ValueDef::serialize(value, serializer)
    }
}

impl<'de> DeserializeAs<'de, Value> for ValueDef {
    fn deserialize_as<D>(deserializer: D) -> Result<Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        ValueDef::deserialize(deserializer)
    }
}
