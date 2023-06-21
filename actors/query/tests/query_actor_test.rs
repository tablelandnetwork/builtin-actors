use fil_actor_query::types::{ConstructorParams, QueryReturn};
use fil_actor_query::{Actor as QueryActor, Method, State};
use fil_actors_runtime::builtin::SYSTEM_ACTOR_ADDR;
use fil_actors_runtime::test_utils::*;
use fvm_ipld_encoding::ipld_block::IpldBlock;
use fvm_shared::address::Address;
use fvm_shared::error::ExitCode;
use fvm_shared::MethodNum;

static DATA: &[u8] = include_bytes!("../testdata/test.db");

#[test]
fn construction() {
    fn construct(db: &[u8], exit_code: ExitCode) {
        let rt = MockRuntime { receiver: Address::new_id(100), ..Default::default() };
        rt.set_caller(*SYSTEM_ACTOR_CODE_ID, SYSTEM_ACTOR_ADDR);
        rt.expect_validate_caller_addr(vec![SYSTEM_ACTOR_ADDR]);

        let epoch = 1234;
        rt.set_epoch(epoch);
        rt.expect_get_randomness_from_beacon(
            fil_actors_runtime::runtime::DomainSeparationTag::EvmPrevRandao,
            epoch,
            Vec::from(*b"prevrandao"),
            [0xff; 32],
        );

        if exit_code.is_success() {
            rt.call::<QueryActor>(
                Method::Constructor as MethodNum,
                IpldBlock::serialize_dag_cbor(&ConstructorParams { db: db.to_vec() }).unwrap(),
            )
            .unwrap();

            let state: State = rt.get_state();
            println!("{:?}", state.db);
            rt.expect_validate_caller_any();

            let ret: IpldBlock =
                rt.call::<QueryActor>(Method::Query as MethodNum, None).unwrap().unwrap();
            let foo = serde_ipld_dagcbor::from_slice::<QueryReturn>(ret.data.as_slice()).unwrap();
            println!("{}", std::str::from_utf8(foo.ret.as_slice()).unwrap());
        } else {
            expect_abort(
                exit_code,
                rt.call::<QueryActor>(1, IpldBlock::serialize_dag_cbor(&db).unwrap()),
            )
        }
        rt.verify();
    }

    construct(DATA, ExitCode::OK);
}
