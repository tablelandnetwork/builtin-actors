use bytes::{Buf, Bytes};
use fvm_ipld_blockstore::{Block, Blockstore};
use fvm_ipld_encoding::ipld_block::IpldBlock;
use fvm_ipld_encoding::CborStore;
use fvm_shared::error::ExitCode;
use fvm_shared::{MethodNum, IPLD_RAW, METHOD_CONSTRUCTOR};
use multihash::Code;
use num_derive::FromPrimitive;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

use fil_actors_runtime::builtin::singletons::SYSTEM_ACTOR_ADDR;
use fil_actors_runtime::runtime::{ActorCode, Runtime};
use fil_actors_runtime::{actor_dispatch, FIRST_EXPORTED_METHOD_NUMBER};
use fil_actors_runtime::{actor_error, ActorError, AsActorError};
use types::{ConstructorParams, QueryReturn};

pub use self::state::State;

mod state;
// pub mod testing;
pub mod types;

#[cfg(feature = "fil-actor")]
fil_actors_runtime::wasm_trampoline!(Actor);

/// Account actor methods available
#[derive(FromPrimitive)]
#[repr(u64)]
pub enum Method {
    Constructor = METHOD_CONSTRUCTOR,
    Query = 2,
}

/// Query Actor
pub struct Actor;

impl Actor {
    pub fn constructor(rt: &impl Runtime, params: ConstructorParams) -> Result<(), ActorError> {
        rt.validate_immediate_caller_is(std::iter::once(&SYSTEM_ACTOR_ADDR))?;

        let db = rt
            .store()
            .put_cbor(&params.db, Code::Blake2b256)
            .context_code(ExitCode::USR_ILLEGAL_STATE, "failed to write db")?;

        rt.create(&State { db })?;
        Ok(())
    }

    pub fn query(rt: &impl Runtime) -> Result<QueryReturn, ActorError> {
        rt.validate_immediate_caller_accept_any()?;
        let st: State = rt.state()?;
        let db: Vec<u8> = rt.store().get_cbor(&st.db).unwrap().unwrap();

        let file = Bytes::from(db.clone());
        let builder = ParquetRecordBatchReaderBuilder::try_new(file).unwrap();
        println!("Converted arrow schema is: {}", builder.schema());
        let mut reader = builder.build().unwrap();
        let record_batch = reader.next().unwrap().unwrap();

        println!("Read {} records.", record_batch.num_rows());

        Ok(QueryReturn { ret: db })
    }

    /// Fallback method for unimplemented method numbers.
    pub fn fallback(
        rt: &impl Runtime,
        method: MethodNum,
        _: Option<IpldBlock>,
    ) -> Result<Option<IpldBlock>, ActorError> {
        rt.validate_immediate_caller_accept_any()?;
        if method >= FIRST_EXPORTED_METHOD_NUMBER {
            Ok(None)
        } else {
            Err(actor_error!(unhandled_message; "invalid method: {}", method))
        }
    }
}

impl ActorCode for Actor {
    type Methods = Method;

    fn name() -> &'static str {
        "Query"
    }

    actor_dispatch! {
        Constructor => constructor,
        Query => query,
        _ => fallback [raw],
    }
}
