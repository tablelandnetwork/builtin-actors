use fvm_ipld_encoding::strict_bytes;
use fvm_ipld_encoding::tuple::*;

#[derive(Debug, Serialize_tuple, Deserialize_tuple)]
#[serde(transparent)]
pub struct ConstructorParams {
    #[serde(with = "strict_bytes")]
    pub db: Vec<u8>,
}

#[derive(Debug, Serialize_tuple, Deserialize_tuple)]
#[serde(transparent)]
pub struct QueryReturn {
    #[serde(with = "strict_bytes")]
    pub ret: Vec<u8>,
}
