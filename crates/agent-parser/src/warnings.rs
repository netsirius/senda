use senda_core::AgentCli;

/// A transpilation warning surfaces a canonical field that the target CLI
/// cannot represent. The frontend renders these as a yellow banner above the
/// editor's Save button.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    pub target: AgentCli,
    pub field_path: String,
    pub message: String,
}
