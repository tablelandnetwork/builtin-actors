mod state;
pub mod types;
mod vfs;

pub use self::state::{State, DB};
use fil_actors_runtime::builtin::singletons::SYSTEM_ACTOR_ADDR;
use fil_actors_runtime::runtime::{ActorCode, Runtime};
use fil_actors_runtime::{actor_dispatch, FIRST_EXPORTED_METHOD_NUMBER};
use fil_actors_runtime::{actor_error, ActorError};
use fvm_ipld_encoding::ipld_block::IpldBlock;
use fvm_shared::{MethodNum, METHOD_CONSTRUCTOR};
use getrandom::register_custom_getrandom;
use getrandom::Error;
use num_derive::FromPrimitive;
use rusqlite::{Connection, OpenFlags, Result};
use sqlite_vfs::{register, Vfs};
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

/// DB actor methods available
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

/// DB Actor
pub struct Actor;

impl Actor {
    pub fn constructor(rt: &impl Runtime, params: ConstructorParams) -> Result<(), ActorError> {
        rt.validate_immediate_caller_is(std::iter::once(&SYSTEM_ACTOR_ADDR))?;

        let db = DB::new(rt.store(), params.db, SQLITE_PAGE_SIZE);
        rt.create(&State { db })?;
        Ok(())
    }

    pub fn query<RT>(rt: &RT) -> Result<QueryReturn, ActorError>
    where
        RT: Runtime,
        RT::Blockstore: Clone,
    {
        rt.validate_immediate_caller_accept_any()?;

        let st: State = rt.state().unwrap();
        let is_new = st.db.pages.len() == 0;

        register("vfs", vfs::PagesVfs::<4096, RT>::new(rt), true).expect("register vfs");
        let conn = Connection::open_with_flags_and_vfs(
            "main.db",
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            "vfs",
        )
        .expect("open connection");

        if is_new {
            conn.execute("PRAGMA page_size = 4096;", []).expect("set page_size = 4096");
        }

        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode = MEMORY", [], |row| row.get(0))
            .expect("set journal_mode = MEMORY");
        assert_eq!(journal_mode, "memory");

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
