# Breadboard

> **For users of Breadboard CLI** | New to Breadboard? Start with [QUICKSTART.md](QUICKSTART.md)

Routing engine that manages token budgets across Claude, Codex, and Cursor CLI.

## What It Does

Breadboard automatically:
- **Classifies** your task (code generation, debugging, chat, etc.)
- **Routes** to the most effective model for that task
- **Enforces** budget limits to prevent subscription overages
- **Tracks** usage across all services

## Quick Start

### 1. Installation

```bash
# Clone and build
git clone https://github.com/USERNAME/breadboard.git
cd breadboard
cargo build --release

# Install to PATH
sudo cp target/release/breadboard /usr/local/bin/

# Verify installation
breadboard --version
```

**Prerequisites:**
- Rust 1.70 or higher
- At least one AI CLI tool installed (Claude, Codex, or Cursor)

**Build Options:**

```bash
# Full build (3.2MB) - includes dashboard and sync
cargo build --release

# Minimal build (2.4MB) - no dashboard or sync
cargo build --release --no-default-features

# Custom build - choose features
cargo build --release --no-default-features --features dashboard
```

Binary sizes:
- **Original**: 8.4MB (unoptimized)
- **Optimized full**: 3.2MB (62% smaller)
- **Minimal**: 2.4MB (71% smaller)

Startup times:
- **Original**: ~470ms
- **Optimized**: ~230ms (50% faster)

See [QUICKSTART.md](QUICKSTART.md) for beginner-friendly guide.

### 2. Run Setup Wizard

```bash
breadboard setup
```

The wizard will:
1. Detect which AI CLI tools you have installed
2. Configure subscription tiers
3. Test routing
4. Show example commands

### 3. Verify Configuration

```bash
# Check system health
breadboard doctor

# Test routing without executing
breadboard --dry-run "write a hello world function"

# Run your first prompt
breadboard "explain how async/await works in JavaScript"
```

## Detailed Setup

### API Keys (Optional - for sync command)

If you want to use `breadboard sync` to verify usage with service APIs:

```bash
# Copy the example file
cp .env.example .env

# Edit with your actual API keys
nano .env
```

The `.env` file should contain:
```bash
ANTHROPIC_API_KEY=sk-ant-api03-your-actual-key-here
OPENAI_API_KEY=sk-your-actual-key-here
```

**Note**: This is only needed for the `sync` command. Regular routing works without API keys.

### Option 1: Subscription Tier-Based (Recommended)

Configure your subscription tiers - breadboard will automatically set limits and track usage across multiple time windows:

```bash
# Set your subscription tiers
breadboard set-tier claude claude-pro
breadboard set-tier cursor cursor-business
breadboard set-tier codex codex-payg

# View limits (shows tier defaults and observed limits)
breadboard limits
```

**Supported tiers:**
- Claude: `claude-free`, `claude-pro`, `claude-team`, `claude-enterprise`
- Cursor: `cursor-free`, `cursor-pro`, `cursor-business`
- Codex: `codex-free`, `codex-payg`, `codex-scale`

Breadboard automatically enforces limits across multiple time windows:
- **Per-minute** rate limits
- **Hourly** quotas
- **Daily** caps
- **Monthly** budgets

### Option 2: Manual Token Budgets

Alternatively, set monthly token budgets directly:

```bash
breadboard set-budget claude 2000000    # 2M tokens/month
breadboard set-budget codex 1500000     # 1.5M tokens/month
breadboard set-budget cursor 2500000    # 2.5M tokens/month
```

### Configure Commands (Optional)

```bash
breadboard config
```

This lets you specify the actual CLI commands for each service if they differ from defaults.

### Model Selection

Breadboard selects a service based on task type, then applies the configured model for that
service. Use `--model` for a one-off override:

```bash
breadboard --model opus "refactor this authentication code"
```

Configure default and task-specific models in `~/.config/breadboard/config.yaml`:

```yaml
models:
  claude:
    default: sonnet
    code_generation: opus
    chat: haiku
  codex:
    default: gpt-5-codex
    debugging: gpt-5-codex
  cursor:
    default: default
```

Model IDs are passed to the selected CLI as `--model <id>`.

To inspect the model catalogs exposed by the installed providers, run:

```bash
breadboard models             # Inspect Claude, Codex, and Cursor
breadboard models codex       # Inspect one provider
breadboard models cursor
```

Codex is queried through its local app-server `model/list` endpoint and Cursor
through its `models` command. Claude's CLI does not expose a non-interactive
account-specific catalog, so Breadboard reports its model-option support and
leaves the final ID choice to the provider's model picker. Discovery is
read-only; it never changes your model configuration.

### Session Resumption

Use a named session to keep a stable Breadboard session key across invocations:

```bash
breadboard --session work "start the refactor"
breadboard --session work "continue the refactor"
breadboard session list
breadboard session delete <session-key>
```

The key is an opaque SHA-256 value scoped to the provider, model, project root, and
session name; the prompt text is not included in it. Breadboard prints the key when
the run starts and closes. `--new-session` requires a new session name; `--dry-run`
previews a key without persisting a session. Provider resume IDs are stored separately
and are only used when that provider exposes compatible resume behavior.

This protects the session identifier, but normal history still stores prompt text
(optionally compressed). Session-key hashing does not provide prompt-history privacy;
that would require a separate history-storage mode.

### Agent Communication

Breadboard includes a local, persistent message bridge for explicit agent-to-agent
coordination. Agents must be registered before they can send or receive messages:

```bash
breadboard bridge agent register planner \
  --provider codex --model default --capability planning
breadboard bridge agent register reviewer \
  --provider claude --model sonnet --capability code-review
breadboard bridge agent list
breadboard bridge send --from planner --to reviewer \
  --task refactor-42 "Review the proposed authentication changes"
breadboard bridge inbox reviewer
breadboard bridge ack reviewer <message-id> --claim-token <claim-token>
breadboard bridge agent heartbeat reviewer
```

Messages are targeted, queued in Breadboard's user-only SQLite database, and remain
pending until acknowledged. Inbox reads atomically claim messages for five minutes and
return a claim token; acknowledgement is scoped to the receiving agent. Expired claims
are retried, while expired messages remain inspectable as terminal records. Payloads are
limited to 64 KiB, and agent leases go offline after fifteen minutes without a heartbeat.
When a named Breadboard session resumes, its pending bridge messages are automatically
included in the next provider prompt and acknowledged only after successful execution.
Delivery is at-least-once: if a provider completes and the process crashes before
acknowledgment, the message may be delivered again, so receivers should make work
idempotent. Agent discovery does not expose session keys. Bridge payloads are treated as
external task context, but they are still visible to the provider as prompt text.
The current transport is local-only and trusted through the operating system; do not
expose the MCP stdio bridge remotely without adding authenticated agent credentials.

For a persistent turn-boundary session, run a supported live provider directly:

```bash
breadboard bridge run --provider codex --session worker --model <model-id>
breadboard bridge run --provider claude --session reviewer --model <model-id>
```

The command keeps the provider process alive, accepts prompts from stdin, and checks the
bridge mailbox between turns. Codex uses its app-server protocol and Claude uses its
streaming JSON protocol. Cursor intentionally remains queued-only until it exposes a
supported bidirectional session transport; `bridge run --provider cursor` rejects rather
than attempting terminal keystroke injection.

### History Compression (Enabled by Default)

Breadboard automatically compresses prompts and outputs before storing them in the database:

- **Long prompts** (>500 chars) are truncated with indicator
- **Token counts** remain accurate (measured before compression)
- **Database size** reduced by ~70-90% for large prompts

Configuration in `~/.config/breadboard/config.yaml`:
```yaml
compress_history: true  # default
```

To disable compression:
```yaml
compress_history: false
```

This feature reduces storage without affecting token tracking accuracy.

## Usage

### Basic routing

```bash
# Just pass your prompt - breadboard routes automatically
breadboard "write a function to parse JSON"
breadboard "debug this error in my code"
breadboard "explain how React hooks work"
```

### Check status

```bash
breadboard status
```

Shows usage and budget for each service with visual progress bars.

### View limits

```bash
breadboard limits
```

Shows all rate limits (per-minute, hourly, daily, monthly) with observed vs tier-based limits.

### View history

```bash
breadboard history --limit 20
```

### Reset usage

```bash
# Reset specific service
breadboard reset --service claude

# Reset all
breadboard reset
```

### Sync with APIs

```bash
breadboard sync
```

Queries actual usage from service APIs and compares with local tracking:
- **Claude**: Anthropic API (placeholder - not available yet)
- **Codex**: OpenAI usage API
- **Cursor**: No public API - local tracking only

Displays discrepancies between API-reported usage and local counts.

**Note**: Requires `ANTHROPIC_API_KEY` and/or `OPENAI_API_KEY` environment variables.

### Interactive Dashboard

```bash
breadboard dashboard
```

Launches a real-time TUI dashboard with:
- Color-coded usage gauges
- Service percentages
- Recent routing history
- Keyboard shortcuts (q=quit, r=refresh)

### Statistics & Analytics

```bash
breadboard stats
```

Shows comprehensive analytics:
- **Service distribution**: Which services you use most
- **Task type distribution**: Breakdown of task types
- **Success rates**: Track command success/failure rates
- **Cache performance**: Tokens and costs saved from caching

### Cost Analysis

```bash
# All-time costs
breadboard costs

# Last 30 days
breadboard costs --days 30
```

Displays:
- Per-service costs in USD
- Total spending
- Cache savings
- Cost distribution percentages

### Export Data

```bash
# Export to default file
breadboard export

# Export to custom file
breadboard export --output my_usage.csv
```

Exports all usage data to CSV for analysis in Excel, Google Sheets, or other tools.

### Service Health

```bash
breadboard health              # Last 24 hours
breadboard health --hours 168  # Last week
```

Monitor service performance:
- Average response times
- Success rates
- Recent failures

### Configuration Profiles

```bash
breadboard profile create work
breadboard profile save work
breadboard profile list
breadboard profile switch work
```

Manage multiple configurations (work/personal/per-client).

### Prompt Templates

```bash
# Save template with variables
breadboard template save review "Review this {{language}} code: {{code}}"

# Use template
breadboard template use review language=python code="def foo(): pass"

# List templates
breadboard template list
```

Reusable prompts with variable substitution.

### Service Aliases

```bash
breadboard alias set gpt4 codex
breadboard alias set sonnet claude
breadboard alias list
```

Custom names for services.

## How Routing Works

**Priority: Effectiveness → Budget**

1. **Task Classification**: Keywords in your prompt determine task type
2. **Effectiveness Ranking**: Each task type has a preferred model order
3. **Budget Check**: Routes to most effective model with available budget
4. **Fallback**: If primary is over budget, tries next best option

### Task Type Rankings

- **Code Generation**: cursor → codex → claude
- **Code Refactoring**: cursor → claude → codex
- **Code Review**: claude → cursor → codex
- **Debugging**: cursor → claude → codex
- **Documentation**: claude → cursor → codex
- **Testing**: cursor → codex → claude
- **Chat/Explanation**: claude → cursor → codex

## Dynamic Limit Detection

Breadboard **self-calibrates** based on actual API responses:

1. **Tier Defaults**: Starts with known limits for your subscription tier
2. **API Observation**: Parses rate limit info from CLI tool outputs
3. **Auto-Update**: Stores observed limits and uses them for future routing
4. **Variance Detection**: Warns if observed limits differ from tier expectations

This means breadboard adapts to:
- API limit changes
- Tier upgrades/downgrades
- Service-specific variations

View observed vs expected limits anytime:
```bash
breadboard limits
```

## Token Tracking

Breadboard parses **actual token usage** from CLI tool outputs instead of estimating.

**Supported formats:**
- `tokens: 1234` or `tokens_used: 1234`
- `input_tokens: 500, output_tokens: 734`
- JSON: `{"usage": {"total_tokens": 1234}}`
- Natural: `Used 1234 tokens`
- Claude format: `input=500 output=734`

**How it works:**
1. Runs your CLI command (claude, codex, cursor)
2. Parses stdout/stderr for token usage patterns
3. Falls back to character estimation if no match
4. Tracks usage across all time windows

## Integration with rtk

Breadboard works great with [rtk](https://github.com/anthropics/rtk):

```bash
# rtk reduces token usage in command outputs
# breadboard manages cumulative budget across services

breadboard "show me the error logs" | rtk summary
```

## Examples

```bash
# Code generation → routes to Cursor
breadboard "create a REST API with Express"

# Explanation → routes to Claude
breadboard "explain the differences between async/await and promises"

# Debugging → routes to Cursor
breadboard "fix the memory leak in this component"

# When over budget, automatically falls back
# Cursor at 95% → routes to Codex instead
```

## Troubleshooting

### Command Not Found

If you get "Command 'claude' not found" or similar:

```bash
# Check system health
breadboard doctor

# Install the missing CLI tool or configure a different service
breadboard set-tier <service> none
```

### All Services Over Budget

```bash
# Check current usage
breadboard status

# Reset usage if needed
breadboard reset

# Or increase budgets
breadboard set-budget <service> <tokens>
```

### Something Not Working?

```bash
# Run health check
breadboard doctor

# View detailed logs
breadboard --verbose "your prompt"
```

For more help, see:
- [SETUP.md](SETUP.md) - Complete setup guide
- [QUICKSTART.md](QUICKSTART.md) - Beginner guide
- [GitHub Issues](https://github.com/USERNAME/breadboard/issues) - Report bugs

## Security & Privacy

### Data Storage

Breadboard stores all data locally on your machine:
- **Database**: `~/Library/Application Support/breadboard/usage.db` (macOS)
- **Permissions**: User-only access (0600 - secure)
- **Contents**: Your prompts (compressed), token usage, routing decisions, timestamps

⚠️ **Privacy Notice**: Your prompts may contain sensitive information (code, credentials, data). The database is:
- Stored locally only
- Not sent to external servers (except to configured AI services when you execute prompts)
- Secured with user-only file permissions

### Security Features

✅ **No unsafe code** - All Rust code is memory-safe
✅ **Command validation** - Shell injection protected
✅ **SQL parameterization** - SQL injection prevented
✅ **CSV sanitization** - Formula injection prevented
✅ **Path validation** - Directory traversal prevented
✅ **No hardcoded secrets** - All credentials via environment

### API Key Management

Store API keys securely:
```bash
# 1. Copy example file
cp .env.example .env

# 2. Edit with your keys
nano .env

# 3. Never commit .env to git (already in .gitignore)
```

### Clearing Data

```bash
# Reset usage counters (keeps history)
breadboard reset

# Clear specific service
breadboard reset --service claude

# Delete all data
rm ~/Library/Application\ Support/breadboard/usage.db
```

### Security Issues

For security vulnerabilities, please **do not** open a public issue.

Instead, see our [Security Policy](SECURITY.md) for responsible disclosure.

## Files

- Config: `~/.config/breadboard/config.yaml`
- Database: `~/Library/Application Support/breadboard/usage.db`

## License

MIT
