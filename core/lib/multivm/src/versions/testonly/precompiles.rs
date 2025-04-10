use circuit_sequencer_api::geometry_config::ProtocolGeometry;
use zksync_test_contracts::TestContract;
use zksync_types::{Address, Execute};

use super::{tester::VmTesterBuilder, TestedVm};
use crate::{
    interface::{InspectExecutionMode, TxExecutionMode, VmInterfaceExt},
    versions::testonly::ContractToDeploy,
    vm_latest::constants::BATCH_COMPUTATIONAL_GAS_LIMIT,
};

pub(crate) fn test_keccak<VM: TestedVm>() {
    // Execute special transaction and check that at least 1000 keccak calls were made.
    let contract = TestContract::precompiles_test().bytecode.to_vec();
    let address = Address::repeat_byte(1);
    let mut vm = VmTesterBuilder::new()
        .with_rich_accounts(1)
        .with_bootloader_gas_limit(BATCH_COMPUTATIONAL_GAS_LIMIT)
        .with_execution_mode(TxExecutionMode::VerifyExecute)
        .with_custom_contracts(vec![ContractToDeploy::account(contract, address)])
        .build::<VM>();

    // calldata for `doKeccak(1000)`.
    let keccak1000_calldata =
        "370f20ac00000000000000000000000000000000000000000000000000000000000003e8";

    let account = &mut vm.rich_accounts[0];
    let tx = account.get_l2_tx_for_execute(
        Execute {
            contract_address: Some(address),
            calldata: hex::decode(keccak1000_calldata).unwrap(),
            value: 0.into(),
            factory_deps: vec![],
        },
        None,
    );
    vm.vm.push_transaction(tx);

    let exec_result = vm.vm.execute(InspectExecutionMode::OneTx);
    assert!(!exec_result.result.is_failed(), "{exec_result:#?}");

    let keccak_count = exec_result.statistics.circuit_statistic.keccak256
        * ProtocolGeometry::latest()
            .config()
            .cycles_per_keccak256_circuit as f32;
    assert!(keccak_count >= 1000.0, "{keccak_count}");
}

pub(crate) fn test_sha256<VM: TestedVm>() {
    // Execute special transaction and check that at least 1000 `sha256` calls were made.
    let contract = TestContract::precompiles_test().bytecode.to_vec();
    let address = Address::repeat_byte(1);
    let mut vm = VmTesterBuilder::new()
        .with_rich_accounts(1)
        .with_bootloader_gas_limit(BATCH_COMPUTATIONAL_GAS_LIMIT)
        .with_execution_mode(TxExecutionMode::VerifyExecute)
        .with_custom_contracts(vec![ContractToDeploy::account(contract, address)])
        .build::<VM>();

    // calldata for `doSha256(1000)`.
    let sha1000_calldata =
        "5d0b4fb500000000000000000000000000000000000000000000000000000000000003e8";

    let account = &mut vm.rich_accounts[0];
    let tx = account.get_l2_tx_for_execute(
        Execute {
            contract_address: Some(address),
            calldata: hex::decode(sha1000_calldata).unwrap(),
            value: 0.into(),
            factory_deps: vec![],
        },
        None,
    );
    vm.vm.push_transaction(tx);

    let exec_result = vm.vm.execute(InspectExecutionMode::OneTx);
    assert!(!exec_result.result.is_failed(), "{exec_result:#?}");

    let sha_count = exec_result.statistics.circuit_statistic.sha256
        * ProtocolGeometry::latest()
            .config()
            .cycles_per_sha256_circuit as f32;
    assert!(sha_count >= 1000.0, "{sha_count}");
}

pub(crate) fn test_ecrecover<VM: TestedVm>() {
    // Execute simple transfer and check that exactly 1 `ecrecover` call was made (it's done during tx validation).
    let mut vm = VmTesterBuilder::new()
        .with_rich_accounts(1)
        .with_bootloader_gas_limit(BATCH_COMPUTATIONAL_GAS_LIMIT)
        .with_execution_mode(TxExecutionMode::VerifyExecute)
        .build::<VM>();

    let account = &mut vm.rich_accounts[0];
    let tx = account.get_l2_tx_for_execute(
        Execute {
            contract_address: Some(account.address),
            calldata: vec![],
            value: 0.into(),
            factory_deps: vec![],
        },
        None,
    );
    vm.vm.push_transaction(tx);

    let exec_result = vm.vm.execute(InspectExecutionMode::OneTx);
    assert!(!exec_result.result.is_failed(), "{exec_result:#?}");

    let ecrecover_count = exec_result.statistics.circuit_statistic.ecrecover
        * ProtocolGeometry::latest()
            .config()
            .cycles_per_ecrecover_circuit as f32;
    assert!((ecrecover_count - 1.0).abs() < 1e-4, "{ecrecover_count}");
}
