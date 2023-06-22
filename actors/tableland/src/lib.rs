pub mod state;
pub mod types;
mod vfs;

use crate::state::{State, DB};
use crate::types::QueryParams;
use fil_actors_runtime::builtin::singletons::SYSTEM_ACTOR_ADDR;
use fil_actors_runtime::runtime::{ActorCode, Runtime};
use fil_actors_runtime::{actor_dispatch, FIRST_EXPORTED_METHOD_NUMBER};
use fil_actors_runtime::{actor_error, ActorError};
use fvm_ipld_encoding::ipld_block::IpldBlock;
use fvm_shared::{MethodNum, METHOD_CONSTRUCTOR};
use getrandom::register_custom_getrandom;
use getrandom::Error;
use num_derive::FromPrimitive;
use rusqlite::{types::Value, Connection, OpenFlags};
use sqlite_vfs::register;
use types::{ConstructorParams, QueryReturn};

#[cfg(feature = "fil-actor")]
fil_actors_runtime::wasm_trampoline!(Actor);

// NOTE (sander): This is a custom randomness function for dependencies that use the
// getrandom crate. I haven't seen this method actually be called in my testing, but
// w/o it, the actor won't compile. The VFS has its own randomness method.
pub fn randomness(buf: &mut [u8]) -> Result<(), Error> {
    // Just return zeros.
    let data = (0..buf.len()).map(|_| ((0 as u128) % 256) as u8).collect::<Vec<_>>();
    buf.copy_from_slice(&data);
    Ok(())
}
register_custom_getrandom!(randomness);

const SQLITE_PAGE_SIZE: usize = 4096;

/// Tableland actor methods available
#[derive(FromPrimitive)]
#[repr(u64)]
pub enum Method {
    Constructor = METHOD_CONSTRUCTOR,
    Query = 2,
}

/// Tableland Actor
pub struct Actor;

impl Actor {
    pub fn constructor(rt: &impl Runtime, params: ConstructorParams) -> Result<(), ActorError> {
        rt.validate_immediate_caller_is(std::iter::once(&SYSTEM_ACTOR_ADDR))?;

        let db = DB::new(rt.store(), params.db.as_slice(), SQLITE_PAGE_SIZE);
        rt.create(&State { db })?;
        Ok(())
    }

    pub fn query<RT>(rt: &RT, params: QueryParams) -> Result<QueryReturn, ActorError>
    where
        RT: Runtime,
        RT::Blockstore: Clone,
    {
        rt.validate_immediate_caller_accept_any()?;
        let conn = new_connection(rt)?;

        let mut stmt = conn.prepare(params.stmt.as_str()).unwrap();
        let cols: Vec<String> = stmt.column_names().iter().map(|c| c.to_string()).collect();
        let num_cols = cols.len();
        let mut res: Vec<Vec<Value>> = vec![];
        let mut rows = stmt.query([]).unwrap();
        while let Some(r) = rows.next().unwrap() {
            let mut row: Vec<Value> = vec![Value::Null; num_cols];
            for c in 0..num_cols {
                row[c] = r.get::<_, Value>(c).unwrap();
            }
            res.push(row);
        }

        Ok(QueryReturn { cols, rows: res })
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
        "Tableland"
    }

    actor_dispatch! {
        Constructor => constructor,
        Query => query,
        _ => fallback [raw],
    }
}

fn new_connection<RT>(rt: &RT) -> Result<Connection, ActorError>
where
    RT: Runtime,
    RT::Blockstore: Clone,
{
    let st: State = rt.state().unwrap();
    let is_new = st.db.pages.len() == 0;

    register("vfs", vfs::PagesVfs::<SQLITE_PAGE_SIZE, RT>::new(rt), true).expect("register vfs");
    let conn = Connection::open_with_flags_and_vfs(
        "main.db",
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        "vfs",
    )
    .expect("open connection");

    if is_new {
        conn.execute(format!("PRAGMA page_size = {};", SQLITE_PAGE_SIZE).as_str(), [])
            .expect(format!("set page_size = {}", SQLITE_PAGE_SIZE).as_str());
    }

    let journal_mode: String = conn
        .query_row("PRAGMA journal_mode = MEMORY", [], |row| row.get(0))
        .expect("set journal_mode = MEMORY");
    assert_eq!(journal_mode, "memory");

    Ok(conn)
}
