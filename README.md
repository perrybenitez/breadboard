# Breadboard

Breadboard is a Rust command-line router for Claude, Codex, and Cursor. It
classifies a prompt, selects an available provider according to the task
ranking, applies configured model and budget rules, runs the provider CLI, and
records local usage.

Breadboard also provides:

- named sessions with stable, opaque session keys;
- provider model discovery where the provider exposes it;
- local usage, history, limits, health, cost, and CSV export commands; and
- a local message bridge for coordinating registered agent sessions.

Breadboard does not provide model access itself. The provider CLIs must already
be installed and authenticated on the machine where Breadboard runs.

## Requirements

- Rust and Cargo with a current stable toolchain;
- at least one installed provider CLI: `claude`, `codex`, or `cursor`; and
- a working provider login for any service you intend to use.

The default provider commands are `claude`, `codex`, and `cursor`. If a command
has a different name or path, configure it with `breadboard config`.

## Installation

Build from source:

```bash
git clone https://github.com/perrybenitez/breadboard.git
cd breadboard
cargo build --release
```

Install the release binary somewhere on your `PATH`:

```bash
install -m 755 target/release/breadboard ~/.local/bin/breadboard
breadboard --version
```

On systems where `~/.local/bin` is not on `PATH`, add it to the shell path or
copy the binary to another directory already on `PATH`. The repository also
contains `install.sh`, which builds a release binary and installs it under
`/usr/local/bin`.

The default build includes the dashboard and usage-sync features. A smaller
build can omit optional features:

```bash
cargo build --release --no-default-features
cargo build --release --no-default-features --features dashboard
```

## First run

Run the setup wizard, then verify the provider commands and routing:

```bash
breadboard setup
breadboard doctor
breadboard --dry-run "write a hello world function"
breadboard "explain how async/await works"
```

Routing requires a configured subscription tier for a provider. Configure only
the providers you want Breadboard to consider:

```bash
breadboard set-tier claude claude-pro
breadboard set-tier codex codex-payg
breadboard set-tier cursor cursor-business
```

Supported tier identifiers are:

- Claude: `claude-free`, `claude-pro`, `claude-team`, `claude-enterprise`;
- Codex: `codex-free`, `codex-payg`, `codex-scale`; and
- Cursor: `cursor-free`, `cursor-pro`, `cursor-business`.

Tier values are Breadboard's configured tracking limits. They are not a live
replacement for provider billing or account status. If you know a monthly
token cap that should be enforced locally, set it explicitly:

```bash
breadboard set-budget claude 2000000
breadboard set-budget codex 1500000
breadboard set-budget cursor 2500000
```

Inspect current usage and limits with:

```bash
breadboard status
breadboard limits
```

## Routing and model selection

For a normal prompt, Breadboard classifies the text into one of these task
types: code generation, refactoring, code review, debugging, documentation,
testing, explanation, or general chat. It tries providers in a task-specific
effectiveness order and skips providers that are unavailable or over budget.

The current default order is:

| Task type | Provider order |
| --- | --- |
| Code generation | Cursor, Codex, Claude |
| Refactoring | Cursor, Claude, Codex |
| Code review | Claude, Cursor, Codex |
| Debugging | Cursor, Claude, Codex |
| Documentation | Claude, Cursor, Codex |
| Testing | Cursor, Codex, Claude |
| Explanation | Claude, Cursor, Codex |
| General chat | Claude, Cursor, Codex |

The classifier is keyword-based. Use `--dry-run` to inspect the selected task,
provider, model, and budget information without invoking a provider:

```bash
breadboard --dry-run "review this function for security issues"
```

### One-off model override

`--model` overrides the configured model for the provider selected for that
prompt:

```bash
breadboard --model sonnet "explain this error"
```

Model identifiers are passed to the provider CLI. Breadboard does not validate
that an identifier exists before execution.

### Task-specific model configuration

The configuration file is YAML. A service-level `default` applies when there
is no task-specific entry; a task-specific entry takes precedence:

```yaml
claude_command: claude
codex_command: codex
cursor_command: cursor
compress_history: true

models:
  claude:
    default: sonnet
    code_review: opus
    chat: haiku
  codex:
    default: codex-model-id
    debugging: codex-debug-model-id
  cursor:
    default: cursor-model-id
    code_generation: cursor-code-model-id
```

Replace the example IDs with values accepted by the installed provider. Model
keys are `code_generation`, `code_refactoring`, `code_review`, `debugging`,
`documentation`, `testing`, `chat`, and `explanation`.

### Discovering available models

Use the provider's local discovery surface before choosing an ID:

```bash
breadboard models
breadboard models claude
breadboard models codex
breadboard models cursor
```

Codex is queried through its local app-server model list. Cursor is queried
through its `models` command. The Claude CLI is checked for model-option
support, but it does not expose an account-specific non-interactive catalog;
its model picker remains authoritative. Discovery is read-only and does not
change configuration.

## Sessions and resume behavior

Use a named session when related prompts should share a Breadboard identity:

```bash
breadboard --session refactor "start the refactor"
breadboard --session refactor "continue the refactor"
breadboard session list
```

The session key is derived as an opaque SHA-256 value from the provider, model,
project root, and session name. Prompt text is not included. The key is printed
when a session starts and closes, and `session list` displays it:

```text
Session key: bb_<opaque-value>
Resume with: breadboard --session refactor "your prompt"
```

The session name is limited to 64 ASCII letters, numbers, `-`, `_`, and `.`.
`--new-session` refuses to overwrite an existing matching session; choose a new
name or delete the old record:

```bash
breadboard --session refactor --new-session "start over"
breadboard session delete <session-key>
```

Breadboard stores provider-specific resume identifiers separately from its
stable key. Provider continuation is therefore provider-dependent. Named
sessions are persisted unless the command is a dry run; `--dry-run` previews
the routing and key without executing or persisting the session.

The hash protects the session identifier from exposing prompt content. It does
not encrypt or remove the prompt history stored locally.

## Agent bridge

The bridge is a persistent, local queue for explicit messages between
registered agents. Register agents before sending messages:

```bash
breadboard bridge agent register planner \
  --provider codex --model default --capability planning
breadboard bridge agent register reviewer \
  --provider claude --model sonnet --capability code-review
breadboard bridge agent list
breadboard bridge status
```

Send, receive, acknowledge, and keep an agent online:

```bash
breadboard bridge send \
  --from planner --to reviewer --task refactor-42 \
  "Review the proposed authentication changes"

breadboard bridge inbox reviewer
breadboard bridge ack reviewer <message-id> --claim-token <claim-token>
breadboard bridge agent heartbeat reviewer
```

Bridge messages are targeted and stored in the local database. The important
delivery rules are:

- payloads are limited to 64 KiB;
- message TTLs are bounded to 30 days;
- an inbox read atomically claims pending messages for five minutes;
- a claim must be acknowledged by the recipient with its exact claim token;
- expired claims become retryable, while expired messages remain inspectable;
- agents are considered offline after 15 minutes without a heartbeat; and
- unregistering an agent makes its pending messages undeliverable.

Delivery is at-least-once. A provider can complete work and the process can
fail before acknowledgment, so a message may be delivered again. Bridge
handlers should be idempotent and should not treat a message ID as proof that
the work has never run.

The bridge also exposes a local MCP-compatible stdio server:

```bash
breadboard bridge mcp
```

The server provides registration, agent discovery, message send/receive,
claim-token acknowledgment, heartbeat, and status tools. It reads JSON-RPC
messages from standard input and writes responses to standard output.

## Live sessions and turn boundaries

For a provider process that remains open between prompts, use a live session:

```bash
breadboard bridge run \
  --provider codex --session worker --model <codex-model-id>

breadboard bridge run \
  --provider claude --session reviewer --model <claude-model-id>
```

Enter one prompt per input line. Press `Ctrl-D` to close the session. Bridge
messages are polled while the process is idle and are injected only at the
boundary before the next provider turn. Breadboard waits for the provider turn
to complete before acknowledging the claimed messages.

Claude live sessions use its streaming JSON interface. Codex live sessions use
its app-server JSON-RPC interface. Cursor does not currently have a supported
bidirectional live transport, so `bridge run --provider cursor` rejects rather
than attempting terminal input injection. Cursor can still participate in the
queued bridge workflow through normal named sessions.

If a live provider turn fails, its claimed bridge messages are not
acknowledged. They can be retried after the claim lease expires. Usage is
recorded before acknowledgment, so an acknowledgment failure favors duplicate
delivery over losing usage accounting.

## Configuration and local data

The interactive configuration command writes the provider commands and default
model IDs:

```bash
breadboard config
```

The paths are selected by the host operating system's standard configuration
and local-data directories. `breadboard doctor` prints the resolved paths. The
files are normally:

- configuration: `<config-directory>/breadboard/config.yaml`;
- database: `<local-data-directory>/breadboard/usage.db`.

On Unix-like systems, the database is created with owner-only read/write
permissions. The database contains usage, routing history, and session/bridge
records. It is not encrypted.

History compression is enabled by default:

```yaml
compress_history: true
```

When enabled, stored prompts longer than 500 characters are truncated for
history, while token accounting uses the original invocation. Set
`compress_history: false` when full prompt history is required. Provider output
is displayed for the current invocation but is not stored in the routing
history record.

### Optional usage synchronization

The default build includes `breadboard sync`, which attempts to reconcile local
usage with provider usage APIs when the relevant credentials are available.
Provider API coverage varies; normal CLI routing does not require these
credentials. If the feature is omitted at build time, the `sync` command is not
available.

## Useful commands

```bash
# Routing and diagnostics
breadboard --help
breadboard --verbose "debug this error"
breadboard doctor
breadboard status
breadboard limits

# History and analysis
breadboard history --limit 20
breadboard stats
breadboard costs --days 30
breadboard health --hours 24
breadboard export --output usage.csv

# Usage maintenance
breadboard reset --service claude
breadboard reset

# Optional features in the default build
breadboard dashboard
breadboard sync
```

Additional commands manage profiles, prompt templates, and service aliases:

```bash
breadboard profile create work
breadboard profile save work
breadboard profile switch work
breadboard template save review "Review this {{language}} code: {{code}}"
breadboard template use review language=rust code="fn main() {}"
breadboard alias set review claude
```

Run `breadboard <command> --help` for command-specific options.

## Security and trust model

Breadboard is designed for local use by trusted processes under the same user
account. The bridge does not authenticate an agent's claimed identity, and the
MCP stdio endpoint has no network authentication. Do not expose it through a
remote or shared transport without adding an authentication and authorization
layer.

Bridge payloads are external task context, but they are inserted into provider
prompts. A message can contain instructions that influence the receiving
provider. Treat bridge senders and payloads as untrusted input at the prompt
level, even though the transport itself is local.

Breadboard executes configured provider commands without a shell. It rejects
configured command strings containing shell metacharacters and validates model
identifiers before invoking a provider. Still configure only binaries you
trust. Prompt history, exported CSV files, and provider output may contain
sensitive source code or credentials; protect or delete them as appropriate.

For security reports, follow [SECURITY.md](SECURITY.md) rather than opening a
public issue with sensitive details.

## Troubleshooting

### A provider command is not found

Check the resolved configuration and provider commands:

```bash
breadboard doctor
breadboard config
```

Install the missing provider CLI, make sure it is on `PATH`, or set its full
trusted path in the configuration.

### Routing says no tier is configured

Set a supported tier for the provider:

```bash
breadboard set-tier claude claude-pro
```

Then inspect the result with `breadboard limits`.

### All providers are unavailable or over budget

Inspect usage and limits, then change the local policy deliberately:

```bash
breadboard status
breadboard limits
breadboard set-budget codex 2000000
```

`breadboard reset` resets local usage counters; it does not change provider
account usage or billing state.

### A model ID fails

Run model discovery for the selected provider and use an ID accepted by that
provider:

```bash
breadboard models <service>
breadboard --model <model-id> "your prompt"
```

For Claude, use the provider's own model picker when the non-interactive
catalog is unavailable.

### Bridge messages appear more than once

This is expected after a provider or process failure because delivery is
at-least-once. Make the receiving operation idempotent and acknowledge only
after the work has completed successfully. A claim token expires with its
five-minute lease; receive the message again to obtain a new token.

### A live session does not start

Run the provider directly to verify its login and command path, then check:

```bash
breadboard doctor
breadboard --verbose "test prompt"
```

Live sessions currently support Claude and Codex only. Cursor is intentionally
queued-only.

## Testing and development

Run the test suite with both the default and minimal feature sets:

```bash
cargo test --all-features
cargo test --no-default-features
cargo build --all-features
```

The tests cover task classification, command and model validation, token
parsing, session identity, database persistence, bridge claims and leases,
live Claude/Codex event parsing, turn-boundary delivery, and the MCP stdio
contract. Real provider process smoke tests require locally installed and
authenticated provider CLIs and are not part of the default suite.

For changes:

1. Keep behavior and security assumptions documented alongside the code.
2. Add or update focused unit tests for parsing, routing, persistence, and
   bridge state transitions.
3. Add integration coverage when changing CLI arguments or provider protocols.
4. Run both feature configurations before submitting a change.
5. Keep secrets, local databases, exports, and build artifacts out of commits.

The main implementation is under `src/`. See [DOCUMENTATION.md](DOCUMENTATION.md)
for the documentation map and [TESTS.md](TESTS.md) for the current test scope.

## License

MIT
