#[macro_use]
pub mod macros;

pub mod baseline_solver;
pub mod conversions;
pub mod current_block;
pub mod ethcontract_error;
pub mod event_handling;
pub mod maintenance;
pub mod metrics;
pub mod recent_block_cache;
pub mod sources;
pub mod subgraph;
pub mod token_info;
pub mod token_pair;

use ethcontract::{
    batch::CallBatch,
    dyns::{DynTransport, DynWeb3},
};

pub type Web3Transport = DynTransport;
pub type Web3 = DynWeb3;
pub type Web3CallBatch = CallBatch<Web3Transport>;

/// anyhow errors are not clonable natively. This is a workaround that creates a new anyhow error
/// based on formatting the error with its inner sources without backtrace.
pub fn clone_anyhow_error(err: &anyhow::Error) -> anyhow::Error {
    anyhow::anyhow!("{:#}", err)
}
