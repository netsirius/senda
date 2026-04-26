//! Per-CLI transpilers. Each implementation knows how to turn a canonical
//! agent into the exact bytes the target CLI expects on disk and reports
//! [`Warning`]s for fields it had to drop.

use senda_core::{AgentCli, CanonicalAgent};

use crate::warnings::Warning;

mod claude_code;
mod copilot;
mod gemini;

pub use claude_code::ClaudeCodeTranspiler;
pub use copilot::CopilotTranspiler;
pub use gemini::GeminiTranspiler;

#[derive(Debug, Clone)]
pub struct TranspileOutput {
    pub filename: String,
    pub contents: String,
    pub warnings: Vec<Warning>,
}

pub trait Transpiler {
    fn target(&self) -> AgentCli;

    fn transpile(&self, canonical: &CanonicalAgent) -> TranspileOutput;
}

/// Pick the transpiler that targets the given CLI.
pub fn for_cli(cli: AgentCli) -> Box<dyn Transpiler> {
    match cli {
        AgentCli::Copilot => Box::new(CopilotTranspiler),
        AgentCli::ClaudeCode => Box::new(ClaudeCodeTranspiler),
        AgentCli::Gemini => Box::new(GeminiTranspiler),
    }
}
