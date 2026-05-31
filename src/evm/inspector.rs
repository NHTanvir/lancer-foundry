use std::sync::{Arc, Mutex};

use revm::{
    interpreter::{CallInputs, CallOutcome, Gas, InstructionResult, Interpreter, InterpreterResult},
    primitives::{Address, Bytes, Log, U256},
    EvmContext, Inspector,
};

use crate::cheatcodes::state::CheatcodeState;
use crate::cheatcodes::dispatch;

/// Magic address that intercepts all cheatcode calls.
pub const CHEATCODE_ADDRESS: Address = Address::new([
    0x71, 0x09, 0x70, 0x9e, 0xcf, 0xa9, 0x1a, 0x80, 0x62, 0x6f,
    0xf3, 0x98, 0x9d, 0x68, 0xf6, 0x7f, 0x5b, 0x1d, 0xd1, 0x2d,
]);

pub struct CheatcodeInspector {
    pub state: Arc<Mutex<CheatcodeState>>,
    pub call_depth: u64,
}

impl CheatcodeInspector {
    pub fn new(state: Arc<Mutex<CheatcodeState>>) -> Self {
        CheatcodeInspector { state, call_depth: 0 }
    }
}

impl<DB: revm::Database> Inspector<DB> for CheatcodeInspector {
    fn call(
        &mut self,
        context: &mut EvmContext<DB>,
        inputs: &mut CallInputs,
    ) -> Option<CallOutcome> {
        self.call_depth += 1;

        // Intercept calls to the cheatcode address
        if inputs.target_address == CHEATCODE_ADDRESS {
            if inputs.input.len() < 4 {
                return Some(CallOutcome {
                    result: InterpreterResult {
                        result: InstructionResult::Revert,
                        output: Bytes::from_static(b"calldata too short"),
                        gas: Gas::new(inputs.gas_limit),
                    },
                    memory_offset: inputs.return_memory_offset.clone(),
                });
            }
            let selector: [u8; 4] = inputs.input[..4].try_into().unwrap();
            let calldata = inputs.input[4..].to_vec();

            let mut state = self.state.lock().unwrap();
            let outcome = dispatch::apply_cheatcode(
                selector,
                &calldata,
                &mut state,
                context,
                inputs.caller,
            );
            drop(state);

            return Some(outcome);
        }

        // Apply prank: override caller
        {
            let mut state = self.state.lock().unwrap();
            if let Some(ref prank) = state.active_prank.clone() {
                if self.call_depth == prank.depth + 1 || state.start_prank.is_some() {
                    inputs.caller = prank.sender;
                    if let Some(origin) = prank.origin {
                        context.env.tx.caller = origin;
                    }
                    // Stop prank after one call if it's a one-shot prank
                    if state.active_prank.is_some() && state.start_prank.is_none() {
                        state.active_prank = None;
                    }
                }
            }

            // Apply mock calls
            let key = (inputs.target_address, inputs.input.clone());
            if let Some(mock) = state.get_mock_call(inputs.target_address, &inputs.input) {
                let (success, data) = mock;
                let ir = if success { InstructionResult::Return } else { InstructionResult::Revert };
                return Some(CallOutcome {
                    result: InterpreterResult {
                        result: ir,
                        output: data,
                        gas: Gas::new(inputs.gas_limit),
                    },
                    memory_offset: inputs.return_memory_offset.clone(),
                });
            }
        }

        None
    }

    fn call_end(
        &mut self,
        _context: &mut EvmContext<DB>,
        _inputs: &CallInputs,
        outcome: CallOutcome,
    ) -> CallOutcome {
        self.call_depth = self.call_depth.saturating_sub(1);

        // Handle expectRevert
        {
            let mut state = self.state.lock().unwrap();
            if let Some(ref expected) = state.expected_revert.clone() {
                if outcome.result.is_revert() {
                    // Check that it matches the expected revert
                    let matches = match expected {
                        ExpectedRevert::Any => true,
                        ExpectedRevert::Bytes(data) => outcome.result.output.starts_with(data),
                        ExpectedRevert::Custom(selector) => {
                            outcome.result.output.len() >= 4
                                && &outcome.result.output[..4] == selector.as_slice()
                        }
                    };
                    if matches {
                        state.expected_revert = None;
                        // Turn the revert into a success
                        return CallOutcome {
                            result: InterpreterResult {
                                result: InstructionResult::Return,
                                output: Bytes::new(),
                                gas: outcome.result.gas,
                            },
                            memory_offset: outcome.memory_offset,
                        };
                    }
                } else if self.call_depth == 0 {
                    // Test ended without reverting — that's a test failure
                    state.expect_revert_failed = true;
                    state.expected_revert = None;
                }
            }
        }

        outcome
    }

    fn log(&mut self, _interp: &mut Interpreter, _context: &mut EvmContext<DB>, log: &Log) {
        let mut state = self.state.lock().unwrap();
        if state.recording_logs {
            state.recorded_logs.push(crate::evm::types::EvmLog {
                address: log.address,
                topics: log.data.topics().to_vec(),
                data: log.data.data.clone(),
            });
        }

        // Check expected emits
        state.check_expected_emit(log);
    }
}

// Re-export the inspector type for use in other modules
pub use CheatcodeInspector as CheatInspector;

#[derive(Debug, Clone)]
pub enum ExpectedRevert {
    Any,
    Bytes(Vec<u8>),
    Custom([u8; 4]),
}
