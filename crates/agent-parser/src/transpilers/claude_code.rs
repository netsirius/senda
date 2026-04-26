use senda_core::{AgentCli, CanonicalAgent};

use crate::warnings::Warning;

use super::{TranspileOutput, Transpiler};

pub struct ClaudeCodeTranspiler;

impl Transpiler for ClaudeCodeTranspiler {
    fn target(&self) -> AgentCli {
        AgentCli::ClaudeCode
    }

    fn transpile(&self, canonical: &CanonicalAgent) -> TranspileOutput {
        let mut warnings = Vec::new();

        if canonical.cli_specific.copilot.is_some() {
            warnings.push(Warning {
                target: AgentCli::ClaudeCode,
                field_path: "copilot".to_string(),
                message: "Copilot-specific fields are ignored when generating Claude Code output."
                    .to_string(),
            });
        }

        let filename = format!(
            "{}.{}",
            canonical.name,
            AgentCli::ClaudeCode.agent_extension()
        );
        let contents = crate::canonical::serialize_canonical(canonical).unwrap_or_default();

        TranspileOutput {
            filename,
            contents,
            warnings,
        }
    }
}
