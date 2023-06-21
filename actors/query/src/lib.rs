mod state;
pub mod types;
pub mod vfs2;

pub use self::state::{State, DB};
use fil_actors_runtime::builtin::singletons::SYSTEM_ACTOR_ADDR;
use fil_actors_runtime::runtime::{ActorCode, Runtime};
use fil_actors_runtime::{actor_dispatch, FIRST_EXPORTED_METHOD_NUMBER};
use fil_actors_runtime::{actor_error, ActorError};
use fvm_ipld_encoding::ipld_block::IpldBlock;
// use fvm_ipld_encoding::CborStore;
// use fvm_shared::error::ExitCode;
use fvm_shared::{MethodNum, METHOD_CONSTRUCTOR};
use getrandom::register_custom_getrandom;
use getrandom::Error;
use num_derive::FromPrimitive;
use rusqlite::{Connection, OpenFlags, Result};
use sqlite_vfs::register;
use types::{ConstructorParams, QueryReturn};

pub fn randomness(buf: &mut [u8]) -> Result<(), Error> {
    let data = (0..buf.len()).map(|_| ((123345678910 as u128) % 256) as u8).collect::<Vec<_>>();
    buf.copy_from_slice(&data);
    Ok(())
}

register_custom_getrandom!(randomness);

#[cfg(feature = "fil-actor")]
fil_actors_runtime::wasm_trampoline!(Actor);

/// Account actor methods available
#[derive(FromPrimitive)]
#[repr(u64)]
pub enum Method {
    Constructor = METHOD_CONSTRUCTOR,
    Query = 2,
}

#[derive(Debug)]
struct Person {
    id: i32,
    name: String,
}

/// Query Actor
pub struct Actor;

impl Actor {
    pub fn constructor(rt: &impl Runtime, params: ConstructorParams) -> Result<(), ActorError> {
        rt.validate_immediate_caller_is(std::iter::once(&SYSTEM_ACTOR_ADDR))?;

        let db = DB::new(rt.store(), params.db, 4096);
        // let db_cid = rt
        //     .store()
        //     .put_cbor(&db, Code::Blake2b256)
        //     .context_code(ExitCode::USR_ILLEGAL_STATE, "failed to write db")?;

        rt.create(&State { db })?;
        Ok(())
    }

    pub fn query<RT>(rt: &RT) -> Result<QueryReturn, ActorError>
    where
        RT: Runtime,
        RT::Blockstore: Clone,
    {
        rt.validate_immediate_caller_accept_any()?;

        // const SQLITE_OK: i32 = 0;
        // const SQLITE_ERROR: i32 = 1;

        // match register("vfs", vfs2::PagesVfs::<4096>::new(), true) {
        //     Ok(_) => SQLITE_OK,
        //     Err(RegisterError::Nul(_)) => return ActorError::unspecified("sqlite error"),
        //     Err(RegisterError::Register(code)) => code,
        // }

        // let is_new = page_count() == 0;

        register("vfs", vfs2::PagesVfs::<4096, RT>::new(rt), true).unwrap();
        let conn = Connection::open_with_flags_and_vfs(
            "main.db",
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            "vfs",
        )
        .unwrap();
        conn.execute_batch(
            r#"
            PRAGMA page_size=4096;
            PRAGMA journal_mode=MEMORY;
            "#,
        )
        .unwrap();

        match conn.execute("CREATE TABLE person (id INTEGER PRIMARY KEY, name TEXT NOT NULL)", []) {
            Ok(s) => println!("created table of size {}", s),
            Err(e) => {
                return Err(ActorError::unspecified(format!(
                    "error creating table {}",
                    e.to_string()
                )))
            }
        }
        let me = Person { id: 0, name: "Steven".to_string() };
        conn.execute("INSERT INTO person (name) VALUES (?1)", [&me.name]).unwrap();

        let mut stmt = conn.prepare("SELECT id, name FROM person").unwrap();
        // let mut stmt = conn.prepare("SELECT * from bar").unwrap();
        let person_iter =
            stmt.query_map([], |row| Ok(Person { id: row.get(0)?, name: row.get(1)? })).unwrap();

        let mut foo: String = String::new();
        for person in person_iter {
            foo = person.unwrap().name;
            break;
        }
        Ok(QueryReturn { ret: foo.as_bytes().to_vec() })
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
