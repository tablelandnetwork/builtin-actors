use fvm_ipld_encoding::ipld_block::IpldBlock;
use fvm_shared::address::Address;
use fvm_shared::error::ExitCode;
use fvm_shared::{MethodNum, IPLD_RAW};

// use fil_actor_query::types::QueryReturn;
use fil_actor_query::types::QueryReturn;
use fil_actor_query::{Actor as QueryActor, Method, State};
use fil_actors_runtime::builtin::SYSTEM_ACTOR_ADDR;
use fil_actors_runtime::test_utils::*;
// use fil_actors_runtime::FIRST_EXPORTED_METHOD_NUMBER;

static DATA: &[u8] = include_bytes!("../testdata/data.parquet");

#[test]
fn construction() {
    fn construct(db: &[u8], exit_code: ExitCode) {
        let rt = MockRuntime { receiver: Address::new_id(100), ..Default::default() };
        rt.set_caller(*SYSTEM_ACTOR_CODE_ID, SYSTEM_ACTOR_ADDR);
        rt.expect_validate_caller_addr(vec![SYSTEM_ACTOR_ADDR]);

        // let params = Some(IpldBlock::serialize(IPLD_RAW, db).unwrap());

        if exit_code.is_success() {
            rt.call::<QueryActor>(
                Method::Constructor as MethodNum,
                IpldBlock::serialize_cbor(db).unwrap(),
            )
            .unwrap();

            let state: State = rt.get_state();
            println!("{:?}", state.db);
            // assert_eq!(state.db, db);
            rt.expect_validate_caller_any();

            let ret: IpldBlock =
                rt.call::<QueryActor>(Method::Query as MethodNum, None).unwrap().unwrap();

            let foo = serde_ipld_dagcbor::from_slice::<QueryReturn>(ret.data.as_slice()).unwrap();
            // .deserialize()
            // .unwrap();
            println!("{}", foo.ret.len());
            // assert_eq!(ret, db);
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

// #[test]
// fn token_receiver() {
//     let rt = MockRuntime { receiver: Address::new_id(100), ..Default::default() };
//     rt.set_caller(*SYSTEM_ACTOR_CODE_ID, SYSTEM_ACTOR_ADDR);
//     rt.expect_validate_caller_addr(vec![SYSTEM_ACTOR_ADDR]);
//
//     let param = Address::new_secp256k1(&[2; fvm_shared::address::SECP_PUB_LEN]).unwrap();
//     rt.call::<AccountActor>(
//         Method::Constructor as MethodNum,
//         IpldBlock::serialize_cbor(&param).unwrap(),
//     )
//     .unwrap();
//
//     rt.set_caller(*EVM_ACTOR_CODE_ID, Address::new_id(1000));
//     rt.expect_validate_caller_any();
//     let ret = rt
//         .call::<AccountActor>(
//             frc42_dispatch::method_hash!("Receive"),
//             IpldBlock::serialize_cbor(&UniversalReceiverParams {
//                 type_: 0,
//                 payload: RawBytes::new(vec![1, 2, 3]),
//             })
//             .unwrap(),
//         )
//         .unwrap();
//     assert!(ret.is_none());
// }

// #[test]
// fn authenticate_message() {
//     let rt = MockRuntime { receiver: Address::new_id(100), ..Default::default() };
//     rt.set_caller(*SYSTEM_ACTOR_CODE_ID, SYSTEM_ACTOR_ADDR);
//
//     let addr = Address::new_secp256k1(&[2; fvm_shared::address::SECP_PUB_LEN]).unwrap();
//     rt.expect_validate_caller_addr(vec![SYSTEM_ACTOR_ADDR]);
//     rt.call::<AccountActor>(
//         Method::Constructor as MethodNum,
//         IpldBlock::serialize_cbor(&addr).unwrap(),
//     )
//     .unwrap();
//
//     let state: State = rt.get_state();
//     assert_eq!(state.address, addr);
//
//     let params = IpldBlock::serialize_cbor(&AuthenticateMessageParams {
//         signature: vec![],
//         message: vec![],
//     })
//     .unwrap();
//
//     // Valid signature
//     rt.expect_validate_caller_any();
//     rt.expect_verify_signature(ExpectedVerifySig {
//         sig: Signature::new_secp256k1(vec![]),
//         signer: addr,
//         plaintext: vec![],
//         result: Ok(()),
//     });
//
//     assert!(rt
//         .call::<AccountActor>(Method::AuthenticateMessageExported as MethodNum, params.clone())
//         .unwrap()
//         .unwrap()
//         .deserialize::<bool>()
//         .unwrap());
//
//     rt.verify();
//
//     // Invalid signature
//     rt.expect_validate_caller_any();
//     rt.expect_verify_signature(ExpectedVerifySig {
//         sig: Signature::new_secp256k1(vec![]),
//         signer: addr,
//         plaintext: vec![],
//         result: Err(anyhow!("bad signature")),
//     });
//     expect_abort_contains_message(
//         ExitCode::USR_ILLEGAL_ARGUMENT,
//         "bad signature",
//         rt.call::<AccountActor>(Method::AuthenticateMessageExported as MethodNum, params.clone()),
//     );
//     rt.verify();
//
//     // Ok to call exported method number
//     rt.expect_validate_caller_any();
//     rt.expect_verify_signature(ExpectedVerifySig {
//         sig: Signature::new_secp256k1(vec![]),
//         signer: addr,
//         plaintext: vec![],
//         result: Ok(()),
//     });
//     assert!(rt
//         .call::<AccountActor>(Method::AuthenticateMessageExported as MethodNum, params)
//         .unwrap()
//         .unwrap()
//         .deserialize::<bool>()
//         .unwrap());
// }

// #[test]
// fn test_fallback() {
//     let rt = MockRuntime { receiver: Address::new_id(100), ..Default::default() };
//     rt.set_caller(*SYSTEM_ACTOR_CODE_ID, SYSTEM_ACTOR_ADDR);
//
//     let addr = Address::new_secp256k1(&[2; fvm_shared::address::SECP_PUB_LEN]).unwrap();
//     rt.expect_validate_caller_addr(vec![SYSTEM_ACTOR_ADDR]);
//     rt.call::<AccountActor>(
//         Method::Constructor as MethodNum,
//         IpldBlock::serialize_cbor(&addr).unwrap(),
//     )
//     .unwrap();
//
//     let state: State = rt.get_state();
//     assert_eq!(state.address, addr);
//
//     // this is arbitrary
//     let params = IpldBlock::serialize_cbor(&vec![1u8, 2u8, 3u8]).unwrap();
//
//     // accept >= 2<<24
//     rt.expect_validate_caller_any();
//     let result = rt.call::<AccountActor>(FIRST_EXPORTED_METHOD_NUMBER, params.clone()).unwrap();
//     assert!(result.is_none());
//
//     rt.expect_validate_caller_any();
//     let result = rt.call::<AccountActor>(FIRST_EXPORTED_METHOD_NUMBER + 1, params.clone()).unwrap();
//     assert!(result.is_none());
//
//     // reject < 1<<24
//     rt.expect_validate_caller_any();
//     let result = rt.call::<AccountActor>(FIRST_EXPORTED_METHOD_NUMBER - 1, params);
//     assert!(result.is_err());
//
//     rt.verify();
// }
