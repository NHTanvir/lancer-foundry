pub mod dispatch;
pub mod selectors;
pub mod state;
pub mod vm_gen;

pub use state::CheatcodeState;

/// The magic address all cheatcode calls are routed to.
pub const CHEATCODE_ADDRESS: revm::primitives::Address = crate::evm::inspector::CHEATCODE_ADDRESS;
