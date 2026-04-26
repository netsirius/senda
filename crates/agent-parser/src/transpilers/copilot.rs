use senda_core::{AgentCli, CanonicalAgent};

use crate::warnings::Warning;

use super::{TranspileOutput, Transpiler};

pub struct CopilotTranspiler;

impl Transpiler for CopilotTranspiler {
    fn target(&self) -> AgentCli {
        AgentCli::Copilot
    }

    fn transpile(&self, canonical: &CanonicalAgent) -> TranspileOutput {
        let mut warnings = Vec::new();

        if canonical.cli_specific.claude_code.is_some() {
            warnings.push(Warning {
                target: AgentCli::Copilot,
                field_path: "claude-code".to_string(),
                message: "Claude Code-specific fields are ignored when generating Copilot output."
                    .to_string(),
            });
        }

        let filename = format!("{}.{}", canonical.name, AgentCli::Copilot.agent_extension());
        // Phase 0 emits a verbatim canonical doc; Phase 1 will rewrite the
        // frontmatter to Copilot's exact schema.
        let contents = crate::canonical::serialize_canonical(canonical).unwrap_or_default();

        TranspileOutput {
            filename,
            contents,
            warnings,
        }
    }
}
