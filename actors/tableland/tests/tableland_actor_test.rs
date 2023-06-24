use fil_actor_tableland::state::State;
use fil_actor_tableland::types::{
    ConstructorParams, ExecuteParams, ExecuteReturn, QueryParams, QueryReturn,
};
use fil_actor_tableland::{Actor as TablelandActor, Method};
use fil_actors_runtime::builtin::SYSTEM_ACTOR_ADDR;
use fil_actors_runtime::test_utils::*;
use fvm_ipld_encoding::ipld_block::IpldBlock;
use fvm_shared::address::Address;
use fvm_shared::error::ExitCode;
use fvm_shared::MethodNum;
use serde_ipld_dagcbor::from_slice;

struct TestDB {
    data: &'static [u8],
    num_pages: usize,
}

static DB: TestDB = TestDB { data: include_bytes!("../testdata/test.db"), num_pages: 224 };
static EMPTY_DB: TestDB = TestDB { data: &[], num_pages: 0 };
// static TILE_DB: TestDB =
//     TestDB { data: include_bytes!("../testdata/tahoe.mbtiles"), num_pages: 224 };

#[test]
fn construction() {
    fn construct(db: &TestDB, exit_code: ExitCode) {
        let rt = MockRuntime { receiver: Address::new_id(100), ..Default::default() };

        rt.set_caller(*SYSTEM_ACTOR_CODE_ID, SYSTEM_ACTOR_ADDR);
        rt.expect_validate_caller_addr(vec![SYSTEM_ACTOR_ADDR]);

        let params =
            IpldBlock::serialize_dag_cbor(&ConstructorParams { db: db.data.to_vec() }).unwrap();

        if exit_code.is_success() {
            rt.call::<TablelandActor>(Method::Constructor as MethodNum, params)
                .expect("construction");

            assert_eq!(rt.get_state::<State>().db.pages.len(), db.num_pages);
        } else {
            expect_abort(
                exit_code,
                rt.call::<TablelandActor>(Method::Constructor as MethodNum, params),
            )
        }
        rt.verify();
    }

    // Create runtime from existing db
    construct(&DB, ExitCode::OK);
    // Create runtime with empty db
    construct(&EMPTY_DB, ExitCode::OK);
}

#[test]
fn execution() {
    fn execute(
        db: &TestDB,
        stmts: Vec<&str>,
        exit_code: ExitCode,
        effected_rows: usize,
        first_run: bool,
    ) {
        let rt = MockRuntime { receiver: Address::new_id(100), ..Default::default() };

        rt.set_caller(*SYSTEM_ACTOR_CODE_ID, SYSTEM_ACTOR_ADDR);
        rt.expect_validate_caller_addr(vec![SYSTEM_ACTOR_ADDR]);
        rt.call::<TablelandActor>(
            Method::Constructor as MethodNum,
            IpldBlock::serialize_dag_cbor(&ConstructorParams { db: db.data.to_vec() }).unwrap(),
        )
        .expect("construction");
        rt.verify();

        rt.expect_validate_caller_any();
        if first_run {
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
        true,
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
fn queries() {
    fn query(db: &TestDB, stmt: &str, exit_code: ExitCode, col_count: usize, row_count: usize) {
        let rt = MockRuntime { receiver: Address::new_id(100), ..Default::default() };

        rt.set_caller(*SYSTEM_ACTOR_CODE_ID, SYSTEM_ACTOR_ADDR);
        rt.expect_validate_caller_addr(vec![SYSTEM_ACTOR_ADDR]);
        rt.call::<TablelandActor>(
            Method::Constructor as MethodNum,
            IpldBlock::serialize_dag_cbor(&ConstructorParams { db: db.data.to_vec() }).unwrap(),
        )
        .expect("construction");
        rt.verify();

        rt.expect_validate_caller_any();

        let params = IpldBlock::serialize_cbor(&QueryParams { stmt: stmt.to_string() }).unwrap();

        if exit_code.is_success() {
            let block: IpldBlock =
                rt.call::<TablelandActor>(Method::Query as MethodNum, params).unwrap().unwrap();
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
