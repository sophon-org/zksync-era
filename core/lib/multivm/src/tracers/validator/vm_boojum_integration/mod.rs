use zk_evm_1_4_0::{
    tracing::{BeforeExecutionData, VmLocalStateData},
    zkevm_opcode_defs::{ContextOpcode, FarCallABI, LogOpcode, Opcode},
};
use zksync_system_constants::KECCAK256_PRECOMPILE_ADDRESS;
use zksync_types::{
    get_code_key, h256_to_address, u256_to_address, u256_to_h256, AccountTreeId, StorageKey, H256,
};

use crate::{
    interface::{
        storage::{StoragePtr, WriteStorage},
        tracer::{TracerExecutionStatus, TracerExecutionStopReason, ViolatedValidationRule},
        Halt,
    },
    tracers::{
        dynamic::vm_1_4_0::DynTracer,
        validator::{
            types::{NewTrustedValidationItems, ValidationTracerMode},
            ValidationRoundResult, ValidationTracer,
        },
    },
    vm_boojum_integration::{
        tracers::utils::{
            computational_gas_price, get_calldata_page_via_abi, print_debug_if_needed, VmHook,
        },
        BootloaderState, SimpleMemory, VmTracer, ZkSyncVmState,
    },
    HistoryMode,
};

impl<H: HistoryMode> ValidationTracer<H> {
    fn check_user_restrictions_vm_boojum_integration<S: WriteStorage>(
        &mut self,
        state: VmLocalStateData<'_>,
        data: BeforeExecutionData,
        memory: &SimpleMemory<H::VmBoojumIntegration>,
        storage: StoragePtr<S>,
    ) -> ValidationRoundResult {
        if self.computational_gas_used > self.computational_gas_limit {
            return Err(ViolatedValidationRule::TookTooManyComputationalGas(
                self.computational_gas_limit,
            ));
        }

        let opcode_variant = data.opcode.variant;
        match opcode_variant.opcode {
            Opcode::FarCall(_) => {
                let packed_abi = data.src0_value.value;
                let call_destination_value = data.src1_value.value;

                let called_address = u256_to_address(&call_destination_value);
                let far_call_abi = FarCallABI::from_u256(packed_abi);

                if called_address == KECCAK256_PRECOMPILE_ADDRESS
                    && far_call_abi.memory_quasi_fat_pointer.length == 64
                {
                    let calldata_page = get_calldata_page_via_abi(
                        &far_call_abi,
                        state.vm_local_state.callstack.current.base_memory_page,
                    );
                    let calldata = memory.read_unaligned_bytes(
                        calldata_page as usize,
                        far_call_abi.memory_quasi_fat_pointer.start as usize,
                        64,
                    );

                    let slot_to_add =
                        self.slot_to_add_from_keccak_call(&calldata, self.user_address);

                    if let Some(slot) = slot_to_add {
                        return Ok(NewTrustedValidationItems {
                            new_allowed_slots: vec![slot],
                            ..Default::default()
                        });
                    }
                } else if called_address != self.user_address {
                    let code_key = get_code_key(&called_address);
                    let code = storage.borrow_mut().read_value(&code_key);

                    if code == H256::zero() {
                        // The users are not allowed to call contracts with no code
                        return Err(ViolatedValidationRule::CalledContractWithNoCode(
                            called_address,
                        ));
                    }
                }
            }
            Opcode::Context(context) => {
                match context {
                    ContextOpcode::Meta => {
                        return Err(ViolatedValidationRule::TouchedDisallowedContext);
                    }
                    ContextOpcode::ErgsLeft => {
                        // TODO (SMA-1168): implement the correct restrictions for the gas left opcode.
                    }
                    _ => {}
                }
            }
            Opcode::Log(LogOpcode::StorageRead) => {
                let key = data.src0_value.value;
                let this_address = state.vm_local_state.callstack.current.this_address;
                let msg_sender = state.vm_local_state.callstack.current.msg_sender;

                if !self.is_allowed_storage_read(storage.clone(), this_address, key, msg_sender) {
                    return Err(ViolatedValidationRule::TouchedDisallowedStorageSlots(
                        this_address,
                        key,
                    ));
                }

                if self.trusted_address_slots.contains(&(this_address, key)) {
                    let storage_key =
                        StorageKey::new(AccountTreeId::new(this_address), u256_to_h256(key));

                    let value = storage.borrow_mut().read_value(&storage_key);

                    return Ok(NewTrustedValidationItems {
                        new_trusted_addresses: vec![h256_to_address(&value)],
                        ..Default::default()
                    });
                }
            }
            _ => {}
        }

        Ok(Default::default())
    }
}

impl<S: WriteStorage, H: HistoryMode> DynTracer<S, SimpleMemory<H::VmBoojumIntegration>>
    for ValidationTracer<H>
{
    fn before_execution(
        &mut self,
        state: VmLocalStateData<'_>,
        data: BeforeExecutionData,
        memory: &SimpleMemory<H::VmBoojumIntegration>,
        storage: StoragePtr<S>,
    ) {
        // For now, we support only validations for users.
        if let ValidationTracerMode::UserTxValidation = self.validation_mode {
            self.computational_gas_used = self
                .computational_gas_used
                .saturating_add(computational_gas_price(state, &data));

            let validation_round_result =
                self.check_user_restrictions_vm_boojum_integration(state, data, memory, storage);
            self.process_validation_round_result(validation_round_result);
        }

        let hook = VmHook::from_opcode_memory(&state, &data);
        print_debug_if_needed(&hook, &state, memory);

        let current_mode = self.validation_mode;
        match (current_mode, hook) {
            (ValidationTracerMode::NoValidation, VmHook::AccountValidationEntered) => {
                // Account validation can be entered when there is no prior validation (i.e. "nested" validations are not allowed)
                self.validation_mode = ValidationTracerMode::UserTxValidation;
            }
            (ValidationTracerMode::NoValidation, VmHook::PaymasterValidationEntered) => {
                // Paymaster validation can be entered when there is no prior validation (i.e. "nested" validations are not allowed)
                self.validation_mode = ValidationTracerMode::PaymasterTxValidation;
            }
            (_, VmHook::AccountValidationEntered | VmHook::PaymasterValidationEntered) => {
                panic!(
                    "Unallowed transition inside the validation tracer. Mode: {:#?}, hook: {:#?}",
                    self.validation_mode, hook
                );
            }
            (_, VmHook::NoValidationEntered) => {
                // Validation can be always turned off
                self.validation_mode = ValidationTracerMode::NoValidation;
            }
            (_, VmHook::ValidationStepEndeded) => {
                // The validation step has ended.
                self.should_stop_execution = true;
            }
            (_, _) => {
                // The hook is not relevant to the validation tracer. Ignore.
            }
        }
    }
}

impl<S: WriteStorage, H: HistoryMode> VmTracer<S, H::VmBoojumIntegration> for ValidationTracer<H> {
    fn finish_cycle(
        &mut self,
        _state: &mut ZkSyncVmState<S, H::VmBoojumIntegration>,
        _bootloader_state: &mut BootloaderState,
    ) -> TracerExecutionStatus {
        if self.should_stop_execution {
            return TracerExecutionStatus::Stop(TracerExecutionStopReason::Finish);
        }
        if let Some(result) = self.result.get() {
            return TracerExecutionStatus::Stop(TracerExecutionStopReason::Abort(
                Halt::TracerCustom(format!("Validation error: {:#?}", result)),
            ));
        }
        TracerExecutionStatus::Continue
    }
}
