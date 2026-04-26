# Format conversion — canonical ↔ Copilot / Claude Code / Gemini

Senda's canonical agent document is a YAML frontmatter + Markdown body. It is
a **superset** of the three CLI formats, so anything an author writes can be
preserved on disk even if a particular target CLI cannot use a given field.
Lossy fields produce warnings, never errors.

## Frontmatter shape

```yaml
---
name: triage-emails
description: Classify incoming emails and draft a response.

# Required. Min 1 CLI from: copilot, claude-code, gemini.
targets: [copilot, claude-code]

# Common fields — copied to every target.
tools: [read_file, write_file, gmail/list_messages]

mcp-servers:
  gmail:
    type: local
    command: gmail-mcp
    args: ["--read-only"]
    env:
      GMAIL_TOKEN: ${secret:gmail_token}

# CLI-specific overrides — applied only to the matching target.
copilot:
  target: github-copilot

claude-code:
  permissionMode: acceptEdits
  hooks:
    on_session_start: notify-team
---
```

## Field mapping

| Canonical field             | Copilot CLI              | Claude Code            | Gemini CLI              |
| --------------------------- | ------------------------ | ---------------------- | ----------------------- |
| File extension              | `.agent.md`              | `.md`                  | `.toml`                 |
| Output folder               | `~/.copilot/agents/`     | `~/.claude/agents/`    | `~/.gemini/agents/`     |
| `name`                      | `name`                   | `name`                 | `[agent].name`          |
| `description`               | `description`            | `description`          | `[agent].description`   |
| `tools`                     | `tools` (verbatim)       | `tools` (verbatim)     | `allowedTools`          |
| `mcp-servers`               | `mcp-servers` (verbatim) | `mcpServers` (camelCase) | `[mcp.<name>]` blocks |
| `copilot.target`            | `target`                 | ⚠ ignored              | ⚠ ignored               |
| `claude-code.permissionMode`| ⚠ ignored                | `permissionMode`       | ⚠ ignored               |
| `claude-code.hooks`         | ⚠ ignored                | `hooks`                | ⚠ partial / ignored     |
| Body Markdown               | verbatim                 | verbatim               | verbatim (in `prompt`)  |

⚠ = warn-and-degrade. The field is dropped from the generated artefact for
that CLI; a [`Warning`](../crates/agent-parser/src/warnings.rs) is recorded
and surfaced in the editor.

## Round-trip guarantees (Phase 0)

* The canonical reader preserves the body verbatim — leading newlines after
  the closing fence are trimmed once, never the body's own indentation.
* Frontmatter round-trips through `serde_yaml`. Field ordering is not
  preserved; structural equivalence is.
* Empty `targets` is rejected at parse time with `ParseError::EmptyTargets`.

Phases 1 and beyond will land the per-CLI emitters that produce real Copilot,
Claude and Gemini output. Today the transpilers emit the canonical doc as a
placeholder so the rest of the pipeline can be exercised.
