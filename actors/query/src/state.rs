use cid::Cid;
use fvm_ipld_encoding::tuple::*;

/// Query actor state
#[derive(Serialize_tuple, Deserialize_tuple, Clone, Debug)]
pub struct State {
    pub db: Cid,
}
