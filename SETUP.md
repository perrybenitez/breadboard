# Breadboard Setup Guide

> **Detailed installation and configuration** | Quick setup? See [QUICKSTART.md](QUICKSTART.md)

## Prerequisites

Before installing Breadboard, ensure you have:

### 1. Rust 1.70+
```bash
# Check if installed:
rustc --version

# Install if needed:
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 2. At least one AI CLI tool

You need at least one of these installed:

- **Claude**: [Installation Guide](https://anthropic.com/claude-code)
- **Codex**: [OpenAI CLI](https://platform.openai.com/)
- **Cursor**: [Download](https://cursor.sh/)

Check which are installed:
```bash
which claude codex cursor
```

---

## Quick Start (5 minutes)

### 1. Clone and Build

```bash
# Clone the repository
git clone https://github.com/USERNAME/breadboard.git
cd breadboard

# Build and install
./install.sh

# Verify installation
breadboard --version
```

### 2. Run Setup Wizard

```bash
breadboard setup
```

The wizard will:
- Detect which AI tools you have installed
- Ask about your subscription tiers
- Test that routing works
- Show you example commands

### 3. Verify Setup

```bash
# Check configuration
breadboard doctor

# Test routing decision
breadboard --dry-run "write a hello world function"

# Run your first prompt
breadboard "explain how async/await works in JavaScript"
```

---

## Manual Configuration (Alternative)

If you prefer to configure manually instead of using the wizard:

### Set Subscription Tiers

```bash
breadboard set-tier claude claude-pro
breadboard set-tier cursor cursor-business
breadboard set-tier codex codex-payg
```

**Available tiers**:
- **Claude**: `claude-free`, `claude-pro`, `claude-team`, `claude-enterprise`
- **Cursor**: `cursor-free`, `cursor-pro`, `cursor-business`
- **Codex**: `codex-free`, `codex-payg`, `codex-scale`

### Configure Custom Commands (Optional)

If your CLI tools aren't named `claude`, `codex`, `cursor`:

```bash
breadboard config
```

---

## API Keys (Optional - for sync command)

If you want to use `breadboard sync` to verify usage with service APIs:

### Option 1: Use .env file

```bash
# Copy the example file
cp .env.example .env

# Edit with your actual API keys
nano .env
```

Your `.env` file:
```bash
ANTHROPIC_API_KEY=sk-ant-api03-your-actual-key-here
OPENAI_API_KEY=sk-your-actual-key-here
```

### Option 2: Export to shell

```bash
export ANTHROPIC_API_KEY="sk-ant-api03-your-key"
export OPENAI_API_KEY="sk-your-key"
```

Add to your `~/.bashrc` or `~/.zshrc` to persist.

**Note**: The `sync` command has limited functionality:
- Claude: No API available yet
- Cursor: No public API
- Codex: Partial support

Skip this unless you specifically need API reconciliation.

---

## Verification

After setup, verify everything works:

### 1. Check Configuration

```bash
breadboard doctor
```

Expected output:
```
Breadboard Health Check

System:
  [OK] Binary: /usr/local/bin/breadboard
  [OK] Database: ~/.../breadboard/usage.db
  [OK] Config: ~/.../breadboard/config.yaml

Services:
  [OK] claude (command: claude)
     Tier: claude-pro
     Usage: 0/5000000 tokens (0.0%)
  [OK] cursor (command: cursor)
     Tier: cursor-pro
     Usage: 0/10000000 tokens (0.0%)

[OK] All systems operational!
```

### 2. Test Routing Decision

```bash
breadboard --dry-run "write a hello world function"
```

Expected output:
```
>> Task: Code Generation
>> Routing to: cursor
[INFO] Dry-run mode - not executing

Current Usage:
  Service: cursor
  Monthly: 0/10000000 tokens (0%)
```

### 3. Run Actual Prompt

```bash
breadboard "explain the difference between let and const in JavaScript"
```

Expected output:
```
>> Task: Explanation
>> Routing to: claude
[Claude's explanation...]
[OK] 1234 tokens used (1234/5000000 - 0%)
```

### 4. Check Usage

```bash
breadboard status
```

Expected output:
```
Token Usage Status

[OK] claude [██░░░░░░░░░░░░░░░░░░░░░░░░░░░░]
   1234/5000000 tokens (0%)

[OK] cursor [░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░]
   0/10000000 tokens (0%)
```

---

## Troubleshooting

### "Command not found: cargo"

**Problem**: Rust is not installed.

**Solution**:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### "Command 'claude' not found"

**Problem**: The CLI tool isn't installed or not in PATH.

**Solutions**:
1. Install the tool (see Prerequisites above)
2. Use a different service: `breadboard set-tier claude none`
3. Set custom command: `breadboard config`

### "No tier configured"

**Problem**: Subscription tier not set.

**Solution**:
```bash
breadboard set-tier <service> <tier>
# Example: breadboard set-tier claude claude-pro
```

### "Database error"

**Problem**: Database file is corrupted.

**Solution**:
```bash
# Backup current data
breadboard export --output backup.csv

# Remove database
rm -rf ~/Library/Application\ Support/breadboard/usage.db

# Reimport (optional)
# Note: Import feature not yet implemented
```

### Dashboard not working

**Problem**: Dashboard requires a terminal (TTY).

**Solution**: Use `breadboard status` instead, or run in a real terminal (not via CI/script).

---

## Configuration Files

Breadboard stores data in standard locations:

### macOS
```
~/Library/Application Support/breadboard/
  └── usage.db              # SQLite database (usage, budgets, history)

~/.config/breadboard/
  └── config.yaml           # Command paths configuration
```

### Linux
```
~/.local/share/breadboard/
  └── usage.db

~/.config/breadboard/
  └── config.yaml
```

### Windows
```
%LOCALAPPDATA%\breadboard\
  └── usage.db

%APPDATA%\breadboard\
  └── config.yaml
```

---

## Next Steps

Once setup is complete, explore these features:

### Daily Usage
```bash
# Route prompts automatically
breadboard "your prompt here"

# Check usage
breadboard status

# Live dashboard
breadboard dashboard
```

### Analytics
```bash
# View statistics
breadboard stats

# Cost analysis
breadboard costs

# Export data
breadboard export
```

### Advanced Features
```bash
# Create profiles for different contexts
breadboard profile create work
breadboard profile save work

# Save reusable templates
breadboard template save review "Review this code: {{code}}"

# Create service aliases
breadboard alias set gpt4 codex
```

See [README.md](README.md) for full documentation.

---

## Getting Help

- Run: `breadboard --help`
- Check health: `breadboard doctor`
- Full guide: [README.md](README.md)
- Report issues: [GitHub Issues](https://github.com/USERNAME/breadboard/issues)
