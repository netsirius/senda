use senda_core::{AgentCli, CanonicalAgent};

use crate::warnings::Warning;

use super::{TranspileOutput, Transpiler};

pub struct GeminiTranspiler;

impl Transpiler for GeminiTranspiler {
    fn target(&self) -> AgentCli {
        AgentCli::Gemini
    }

    fn transpile(&self, canonical: &CanonicalAgent) -> TranspileOutput {
        let mut warnings = Vec::new();

        // Gemini ships CLI-specific shapes for several common fields. Phase 0
        // captures the obvious mismatches and degrades; Phase 1 emits TOML.
        if canonical.cli_specific.copilot.is_some() {
            warnings.push(Warning {
                target: AgentCli::Gemini,
                field_path: "copilot".to_string(),
                message: "Copilot-specific fields are ignored when generating Gemini output."
                    .to_string(),
            });
        }
        if canonical.cli_specific.claude_code.is_some() {
            warnings.push(Warning {
                target: AgentCli::Gemini,
                field_path: "claude-code".to_string(),
                message: "Claude Code-specific fields are ignored when generating Gemini output."
                    .to_string(),
            });
        }

        let filename = format!("{}.{}", canonical.name, AgentCli::Gemini.agent_extension());
        let contents = format!("# Phase 0 stub for {}\n", canonical.name);
        TranspileOutput {
            filename,
            contents,
            warnings,
        }
    }
}
