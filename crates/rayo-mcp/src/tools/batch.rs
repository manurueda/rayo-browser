//! rayo_batch tool: execute multiple actions in one MCP call.
//!
//! THE speed multiplier for AI agents.
//! 7 actions in 1 call vs 7 separate MCP round-trips = ~5-7x faster.

use rayo_core::batch::{BatchAction, BatchResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct BatchInput {
    pub actions: Vec<BatchAction>,
}

#[derive(Debug, Serialize)]
pub struct BatchOutput {
    #[serde(flatten)]
    pub result: BatchResult,
}
