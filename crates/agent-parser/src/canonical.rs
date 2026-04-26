//! Canonical document = YAML frontmatter (`---` delimited) + Markdown body.
//!
//! We parse the frontmatter into [`CanonicalAgent`] and store everything after
//! the closing fence as the body. The body is preserved verbatim so we never
//! lose author formatting on round-trips.

use senda_core::CanonicalAgent;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("document is missing the leading `---` frontmatter fence")]
    MissingOpeningFence,
    #[error("document is missing the closing `---` frontmatter fence")]
    MissingClosingFence,
    #[error("invalid YAML in frontmatter: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("`targets:` must contain at least one CLI")]
    EmptyTargets,
}

const FENCE: &str = "---";

/// Parse a canonical agent document. Body is everything after the closing fence,
/// with leading newlines trimmed.
pub fn parse_canonical(input: &str) -> Result<CanonicalAgent, ParseError> {
    let mut lines = input.lines();
    match lines.next() {
        Some(first) if first.trim_end() == FENCE => {}
        _ => return Err(ParseError::MissingOpeningFence),
    }

    let mut frontmatter = String::new();
    let mut body_lines: Vec<&str> = Vec::new();
    let mut closed = false;

    for line in lines {
        if !closed {
            if line.trim_end() == FENCE {
                closed = true;
                continue;
            }
            frontmatter.push_str(line);
            frontmatter.push('\n');
        } else {
            body_lines.push(line);
        }
    }

    if !closed {
        return Err(ParseError::MissingClosingFence);
    }

    #[derive(serde::Deserialize)]
    struct Frontmatter {
        #[serde(flatten)]
        rest: serde_yaml::Value,
    }
    let parsed: Frontmatter = serde_yaml::from_str(&frontmatter)?;
    let mut canonical: CanonicalAgent = serde_yaml::from_value(parsed.rest)?;
    canonical.body = body_lines.join("\n").trim_start_matches('\n').to_string();

    if canonical.targets.is_empty() {
        return Err(ParseError::EmptyTargets);
    }

    Ok(canonical)
}

/// Serialize a [`CanonicalAgent`] back to a canonical document.
pub fn serialize_canonical(agent: &CanonicalAgent) -> Result<String, serde_yaml::Error> {
    // Emit frontmatter without the body field — body lives outside the YAML.
    let value = serde_yaml::to_value(agent)?;
    let frontmatter = if let serde_yaml::Value::Mapping(mut map) = value {
        map.remove(serde_yaml::Value::String("body".to_string()));
        serde_yaml::to_string(&serde_yaml::Value::Mapping(map))?
    } else {
        serde_yaml::to_string(&value)?
    };

    let mut out = String::with_capacity(frontmatter.len() + agent.body.len() + 16);
    out.push_str(FENCE);
    out.push('\n');
    out.push_str(frontmatter.trim_end());
    out.push('\n');
    out.push_str(FENCE);
    out.push_str("\n\n");
    out.push_str(&agent.body);
    if !agent.body.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use senda_core::AgentCli;

    #[test]
    fn parses_minimal_canonical_document() {
        let input = indoc! {"
            ---
            name: triage
            description: Classify incoming tickets.
            targets: [copilot, claude-code]
            ---

            # Body

            Hello.
        "};
        let parsed = parse_canonical(input).unwrap();
        assert_eq!(parsed.name, "triage");
        assert_eq!(
            parsed.targets,
            vec![AgentCli::Copilot, AgentCli::ClaudeCode]
        );
        assert!(parsed.body.contains("# Body"));
    }

    #[test]
    fn rejects_missing_targets() {
        let input = indoc! {"
            ---
            name: foo
            description: bar
            targets: []
            ---

            body
        "};
        assert!(matches!(
            parse_canonical(input),
            Err(ParseError::EmptyTargets)
        ));
    }

    #[test]
    fn rejects_missing_closing_fence() {
        let input = "---\nname: foo\ndescription: bar\ntargets: [copilot]\n";
        assert!(matches!(
            parse_canonical(input),
            Err(ParseError::MissingClosingFence)
        ));
    }

    #[test]
    fn round_trips_a_canonical_document() {
        let input = indoc! {"
            ---
            name: triage
            description: Classify incoming tickets.
            targets:
            - copilot
            tools: [read_file]
            ---

            Body content here.
        "};
        let parsed = parse_canonical(input).unwrap();
        let serialized = serialize_canonical(&parsed).unwrap();
        let reparsed = parse_canonical(&serialized).unwrap();
        assert_eq!(reparsed.name, parsed.name);
        assert_eq!(reparsed.targets, parsed.targets);
        assert_eq!(reparsed.tools, parsed.tools);
        assert_eq!(reparsed.body.trim(), parsed.body.trim());
    }
}
