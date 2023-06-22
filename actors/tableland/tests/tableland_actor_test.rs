use fil_actor_tableland::types::{ConstructorParams, QueryParams, QueryReturn};
use fil_actor_tableland::{Actor as TablelandActor, Method};
use fil_actors_runtime::builtin::SYSTEM_ACTOR_ADDR;
use fil_actors_runtime::test_utils::*;
use fvm_ipld_encoding::ipld_block::IpldBlock;
use fvm_shared::address::Address;
use fvm_shared::error::ExitCode;
use fvm_shared::MethodNum;
use serde_ipld_dagcbor::from_slice;

static DATA: &[u8] = include_bytes!("../testdata/test.db");

#[test]
fn construction() {
    fn construct(db: &[u8], exit_code: ExitCode) {
        let rt = MockRuntime { receiver: Address::new_id(100), ..Default::default() };
        rt.set_caller(*SYSTEM_ACTOR_CODE_ID, SYSTEM_ACTOR_ADDR);
        rt.expect_validate_caller_addr(vec![SYSTEM_ACTOR_ADDR]);

        // let epoch = 1234;
        // rt.set_epoch(epoch);
        // rt.expect_get_randomness_from_beacon(
        //     fil_actors_runtime::runtime::DomainSeparationTag::EvmPrevRandao,
        //     epoch,
        //     Vec::from(*b"prevrandao"),
        //     [0xff; 32],
        // );

        if exit_code.is_success() {
            rt.call::<TablelandActor>(
                Method::Constructor as MethodNum,
                IpldBlock::serialize_dag_cbor(&ConstructorParams { db: db.to_vec() }).unwrap(),
            )
            .unwrap();

            // let state: State = rt.get_state();
            // println!("{:?}", state.db);
            rt.expect_validate_caller_any();

            let ret: IpldBlock = rt
                .call::<TablelandActor>(
                    Method::Query as MethodNum,
                    IpldBlock::serialize_cbor(&QueryParams {
                        stmt: "select \
                        Track.Name, Track.Composer, Track.Milliseconds, Track.Bytes, Track.UnitPrice, Genre.Name as GenreName \
                        from Track \
                        join Genre on Track.GenreId = Genre.GenreId \
                        limit 10;".to_string(),
                    })
                    .unwrap(),
                )
                .unwrap()
                .unwrap();
            let foo = from_slice::<QueryReturn>(ret.data.as_slice()).unwrap();
            println!("{}", serde_json::to_string(&foo).unwrap());
        } else {
            expect_abort(
                exit_code,
                rt.call::<TablelandActor>(1, IpldBlock::serialize_dag_cbor(&db).unwrap()),
            )
        }
        rt.verify();
    }

    construct(DATA, ExitCode::OK);
}
