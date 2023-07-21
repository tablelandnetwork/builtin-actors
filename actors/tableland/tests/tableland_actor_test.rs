use fil_actor_tableland::Actor as TablelandActor;
use fil_actor_tableland_interface::{
    ConstructorParams, ExecuteParams, ExecuteReturn, Method, QueryParams, QueryReturn, State,
};
use fil_actors_runtime::builtin::SYSTEM_ACTOR_ADDR;
use fil_actors_runtime::test_utils::*;
use fvm_ipld_encoding::ipld_block::IpldBlock;
use fvm_shared::address::Address;
use fvm_shared::error::ExitCode;
use fvm_shared::MethodNum;
use serde_ipld_dagcbor::from_slice;
use serial_test::serial;

struct TestDB {
    data: &'static [u8],
    page_count: usize,
    tree_height: usize,
    tree_length: usize,
}

static DB: TestDB = TestDB {
    data: include_bytes!("../testdata/test.db"),
    page_count: 224,
    tree_height: 0,
    tree_length: 224,
};
static TILE_DB: TestDB = TestDB {
    data: include_bytes!("../testdata/tahoe.mbtiles"),
    page_count: 18581,
    tree_height: 1,
    tree_length: 73,
};
static EMPTY_DB: TestDB = TestDB { data: &[], page_count: 0, tree_height: 0, tree_length: 0 };

const BUCKET_SIZE: usize = 256; // IPLD block limit w/ standard page size of 4096

#[test]
#[serial]
fn construction() {
    fn construct(db: &TestDB, exit_code: ExitCode) {
        let rt = MockRuntime { receiver: Address::new_id(100), ..Default::default() };

        rt.set_caller(*SYSTEM_ACTOR_CODE_ID, SYSTEM_ACTOR_ADDR);
        rt.expect_validate_caller_addr(vec![SYSTEM_ACTOR_ADDR]);

        let params = IpldBlock::serialize_dag_cbor(&ConstructorParams {
            db: db.data.to_vec(),
            buck_size: BUCKET_SIZE,
        })
        .unwrap();

        if exit_code.is_success() {
            rt.call::<TablelandActor>(Method::Constructor as MethodNum, params)
                .expect("construction");
            let st = rt.get_state::<State>();

            assert_eq!(st.db.page_count, db.page_count);
            assert_eq!(st.db.tree_height, db.tree_height);
            assert_eq!(st.db.page_tree.len(), db.tree_length);
        } else {
            expect_abort(
                exit_code,
                rt.call::<TablelandActor>(Method::Constructor as MethodNum, params),
            )
        }
        rt.verify();
    }

    // Create runtime from existing dbs
    construct(&DB, ExitCode::OK);
    construct(&TILE_DB, ExitCode::OK);
    // Create runtime with empty db
    construct(&EMPTY_DB, ExitCode::OK);
}

#[test]
#[serial]
fn execution() {
    fn execute(
        db: &TestDB,
        stmts: Vec<&str>,
        exit_code: ExitCode,
        effected_rows: usize,
        need_beacon_randomness: bool,
    ) {
        let rt = MockRuntime { receiver: Address::new_id(100), ..Default::default() };

        rt.set_caller(*SYSTEM_ACTOR_CODE_ID, SYSTEM_ACTOR_ADDR);
        rt.expect_validate_caller_addr(vec![SYSTEM_ACTOR_ADDR]);
        rt.call::<TablelandActor>(
            Method::Constructor as MethodNum,
            IpldBlock::serialize_dag_cbor(&ConstructorParams {
                db: db.data.to_vec(),
                buck_size: BUCKET_SIZE,
            })
            .unwrap(),
        )
        .expect("construction");
        rt.verify();

        rt.expect_validate_caller_any();
        // NOTE (sander): This is a hack. Try expecting randomness to be called
        // for all tests and you'll see that, for some reason probably related to
        // me removing the Sync requirement on the VFS, the tests share some aspect
        // of the underlying SQLite connection, even though each test has its own
        // VFS instance. TL;DR, the journal header is written only once during the
        // first test run, which calls the random method on VFS. This is also the
        // reason why all tests are run serial.
        // I'm not 100% sure, but this shouldn't be an issue when compiling to WASM.
        if need_beacon_randomness {
            let epoch = 1234;
            rt.set_epoch(epoch);
            rt.expect_get_randomness_from_beacon(
                fil_actors_runtime::runtime::DomainSeparationTag::EvmPrevRandao,
                epoch,
                Vec::from(*b"prevrandao"),
                [0xff; 32],
            );
        }

        let params = IpldBlock::serialize_cbor(&ExecuteParams {
            stmts: stmts.iter().map(|s| s.to_string()).collect(),
        })
        .unwrap();

        if exit_code.is_success() {
            let block: IpldBlock =
                rt.call::<TablelandActor>(Method::Execute as MethodNum, params).unwrap().unwrap();
            let ret = from_slice::<ExecuteReturn>(block.data.as_slice()).unwrap();
            assert_eq!(ret.effected_rows, effected_rows);
        } else {
            expect_abort(exit_code, rt.call::<TablelandActor>(Method::Execute as MethodNum, params))
        }
        rt.verify();
    }

    // Create and insert on existing db
    execute(
        &DB,
        vec![
            "create table my_table(id integer primary key, msg text);",
            "insert into my_table(msg) values('hello');",
            "insert into my_table(msg) values('world');",
        ],
        ExitCode::OK,
        2,
        false,
    );
    // Create and insert on empty db
    execute(
        &EMPTY_DB,
        vec![
            "create table my_table(id integer primary key, msg text)",
            "insert into my_table(msg) values('hello')",
            "insert into my_table(msg) values('world')",
        ],
        ExitCode::OK,
        2,
        false,
    );
    // Insert on empty db
    execute(
        &EMPTY_DB,
        vec!["insert into my_table(msg) values('hello');"],
        ExitCode::USR_ILLEGAL_STATE,
        0,
        false,
    );
}

#[test]
#[serial]
fn queries() {
    fn query(db: &TestDB, stmt: &str, exit_code: ExitCode, col_count: usize, row_count: usize) {
        let rt = MockRuntime { receiver: Address::new_id(100), ..Default::default() };

        rt.set_caller(*SYSTEM_ACTOR_CODE_ID, SYSTEM_ACTOR_ADDR);
        rt.expect_validate_caller_addr(vec![SYSTEM_ACTOR_ADDR]);
        rt.call::<TablelandActor>(
            Method::Constructor as MethodNum,
            IpldBlock::serialize_dag_cbor(&ConstructorParams {
                db: db.data.to_vec(),
                buck_size: BUCKET_SIZE,
            })
            .unwrap(),
        )
        .expect("construction");
        rt.verify();

        rt.expect_validate_caller_any();

        let params = IpldBlock::serialize_cbor(&QueryParams { stmt: stmt.to_string() }).unwrap();

        if exit_code.is_success() {
            let block = rt.call::<TablelandActor>(Method::Query as MethodNum, params);
            let block = block.unwrap().unwrap();
            let ret = from_slice::<QueryReturn>(block.data.as_slice()).unwrap();
            assert_eq!(ret.cols.len(), col_count);
            assert_eq!(ret.rows.len(), row_count);
        } else {
            expect_abort(exit_code, rt.call::<TablelandActor>(Method::Query as MethodNum, params))
        }
        rt.verify();
    }

    // Query existing db
    query(
        &DB,
        "select \
            Track.Name, Track.Composer, Track.Milliseconds, Track.Bytes, Track.UnitPrice, Genre.Name as GenreName \
            from Track \
            join Genre on Track.GenreId = Genre.GenreId \
            limit 10;",
        ExitCode::OK,
        6,
        10,
    );
    // Query empty db
    query(&EMPTY_DB, "select * from not_a_table;", ExitCode::USR_ILLEGAL_STATE, 0, 0);
}
