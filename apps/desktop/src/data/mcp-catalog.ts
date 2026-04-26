/**
 * Curated catalog of MCP servers distributed as Docker images on Docker Hub
 * under the `mcp/*` namespace. Each entry knows the env vars the image
 * expects, so the Add MCP form can render real password / URL inputs and
 * generate the right `docker run` incantation.
 *
 * Sources of truth — Docker Hub readmes:
 *   https://hub.docker.com/u/mcp
 *   https://github.com/modelcontextprotocol/servers
 *
 * When in doubt about an image's env contract, treat the readme on Docker
 * Hub as authoritative; this list reflects the consensus at time of
 * writing.
 */

export type EnvKind = "text" | "password" | "url";

export interface CatalogEnvVar {
  key: string;
  label: string;
  kind: EnvKind;
  required: boolean;
  hint?: string;
}

export interface CatalogEntry {
  id: string;
  name: string;
  category: string;
  description: string;
  image: string;
  /** Extra args after the image name (rare). */
  trailingArgs?: string[];
  env: CatalogEnvVar[];
  /** Docs URL the user can read for setup specifics. */
  docs: string;
}

export const MCP_CATALOG: CatalogEntry[] = [
  {
    id: "atlassian",
    name: "Atlassian (Jira + Confluence)",
    category: "Project management",
    description:
      "Jira issues, Confluence pages, JQL search. Works against Cloud and Server.",
    image: "mcp/atlassian",
    docs: "https://hub.docker.com/r/mcp/atlassian",
    env: [
      {
        key: "ATLASSIAN_BASE_URL",
        label: "Base URL",
        kind: "url",
        required: true,
        hint: "https://acme.atlassian.net",
      },
      {
        key: "ATLASSIAN_USER_EMAIL",
        label: "User email",
        kind: "text",
        required: true,
      },
      {
        key: "ATLASSIAN_API_TOKEN",
        label: "API token",
        kind: "password",
        required: true,
        hint: "id.atlassian.com/manage-profile/security/api-tokens",
      },
    ],
  },
  {
    id: "github",
    name: "GitHub",
    category: "Engineering",
    description:
      "Repos, issues, PRs, file contents, search. Read-only by default; PAT scopes determine writes.",
    image: "mcp/github",
    docs: "https://hub.docker.com/r/mcp/github",
    env: [
      {
        key: "GITHUB_PERSONAL_ACCESS_TOKEN",
        label: "Personal access token",
        kind: "password",
        required: true,
      },
    ],
  },
  {
    id: "linear",
    name: "Linear",
    category: "Project management",
    description: "Linear issues, projects, comments. Cloud only.",
    image: "mcp/linear",
    docs: "https://github.com/jerhadf/linear-mcp-server",
    env: [
      {
        key: "LINEAR_API_KEY",
        label: "API key",
        kind: "password",
        required: true,
        hint: "linear.app/settings/api",
      },
    ],
  },
  {
    id: "slack",
    name: "Slack",
    category: "Inbox",
    description: "Read channels, search messages, post replies as a bot.",
    image: "mcp/slack",
    docs: "https://hub.docker.com/r/mcp/slack",
    env: [
      {
        key: "SLACK_BOT_TOKEN",
        label: "Bot token (xoxb-…)",
        kind: "password",
        required: true,
      },
      {
        key: "SLACK_TEAM_ID",
        label: "Team / Workspace ID",
        kind: "text",
        required: false,
      },
    ],
  },
  {
    id: "postgres",
    name: "Postgres (read-only)",
    category: "Data",
    description:
      "Run read-only queries against a Postgres database. Useful for triage agents that need to enrich tickets.",
    image: "mcp/postgres",
    docs: "https://hub.docker.com/r/mcp/postgres",
    trailingArgs: ["${POSTGRES_URL}"],
    env: [
      {
        key: "POSTGRES_URL",
        label: "Connection string",
        kind: "url",
        required: true,
        hint: "postgresql://user:pass@host:5432/dbname",
      },
    ],
  },
  {
    id: "sqlite",
    name: "SQLite",
    category: "Data",
    description: "Query a local SQLite database. Senda mounts the parent dir into the container.",
    image: "mcp/sqlite",
    docs: "https://hub.docker.com/r/mcp/sqlite",
    env: [
      {
        key: "SQLITE_PATH",
        label: "DB path inside container",
        kind: "text",
        required: true,
        hint: "/data/app.db",
      },
    ],
  },
  {
    id: "google-drive",
    name: "Google Drive",
    category: "Inbox",
    description: "List, read and search Drive files. OAuth via gcloud auth.",
    image: "mcp/google-drive",
    docs: "https://hub.docker.com/r/mcp/google-drive",
    env: [
      {
        key: "GDRIVE_CREDENTIALS",
        label: "OAuth credentials JSON (path)",
        kind: "text",
        required: true,
      },
    ],
  },
  {
    id: "sentry",
    name: "Sentry",
    category: "Engineering",
    description: "Errors, issues, alerts. Read-only against your org.",
    image: "mcp/sentry",
    docs: "https://hub.docker.com/r/mcp/sentry",
    env: [
      {
        key: "SENTRY_AUTH_TOKEN",
        label: "Auth token",
        kind: "password",
        required: true,
      },
      {
        key: "SENTRY_ORG",
        label: "Organization slug",
        kind: "text",
        required: true,
      },
    ],
  },
  {
    id: "git",
    name: "Git (filesystem)",
    category: "Engineering",
    description: "Run git commands against a repository on disk. Senda mounts the repo path.",
    image: "mcp/git",
    docs: "https://hub.docker.com/r/mcp/git",
    env: [
      {
        key: "GIT_REPO_PATH",
        label: "Host repo path",
        kind: "text",
        required: true,
        hint: "/Users/me/code/myrepo",
      },
    ],
  },
  {
    id: "filesystem",
    name: "Filesystem (sandboxed)",
    category: "Data",
    description:
      "Read / write files inside a sandboxed mount. Useful when an agent needs scratch space.",
    image: "mcp/filesystem",
    docs: "https://hub.docker.com/r/mcp/filesystem",
    env: [
      {
        key: "FS_ROOT",
        label: "Host root path",
        kind: "text",
        required: true,
        hint: "/tmp/senda-sandbox",
      },
    ],
  },
];

/**
 * Generate the docker run argv for a given catalog entry. The `-e KEY` form
 * (without `=value`) tells Docker to pass through whatever value the calling
 * env exports — Senda exports each KEY into the MCP's env map at config
 * write time, so the variable is present when the CLI spawns the docker
 * subprocess.
 */
export function dockerArgsFor(entry: CatalogEntry): string[] {
  const args = ["run", "-i", "--rm"];
  for (const v of entry.env) {
    args.push("-e", v.key);
  }
  args.push(entry.image);
  if (entry.trailingArgs) {
    args.push(...entry.trailingArgs);
  }
  return args;
}
