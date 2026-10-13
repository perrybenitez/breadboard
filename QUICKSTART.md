# Breadboard Quick Start for Beginners

> **For first-time users** | Already familiar? See [README.md](README.md) for full reference

New to Breadboard? This guide assumes you've never used it before.

## What is Breadboard?

Breadboard is a command-line tool that:
- Sends your prompts to the best AI service (Claude, Codex, or Cursor)
- Tracks how many tokens you've used
- Prevents you from going over your subscription limits

Think of it like a traffic controller for AI services.

## Do I Need This?

✅ **You need Breadboard if:**
- You have multiple AI CLI tools installed
- You want to stay under token limits
- You want automatic routing to the best tool for each task

❌ **You DON'T need Breadboard if:**
- You only use one AI service
- You don't care about tracking usage
- You have unlimited subscriptions

## Installation Steps

### Step 1: Install Prerequisites

#### 1a. Install Rust

Visit https://rustup.rs/ and follow the instructions for your OS.

Verify installation:
```bash
rustc --version
# Should show: rustc 1.70 or higher
```

#### 1b. Install at least one AI CLI tool

You need at least one of these:
- **Claude**: Follow guide at anthropic.com/claude-code
- **Codex**: Follow guide at platform.openai.com/
- **Cursor**: Download from cursor.sh/

Test your tools:
```bash
claude "hello"   # Should respond
cursor "hello"   # Should respond
codex "hello"    # Should respond
```

### Step 2: Install Breadboard

```bash
# 1. Download Breadboard
git clone https://github.com/USERNAME/breadboard.git
cd breadboard

# 2. Build and install
./install.sh

# 3. Verify it worked
breadboard --version
# Should show: breadboard 0.1.0
```

### Step 3: Run Setup Wizard

```bash
breadboard setup
```

The wizard will:
1. Detect which AI tools you have installed
2. Ask about your subscription tiers
3. Test that routing works
4. Show you example commands

Just follow the prompts and answer the questions.

### Step 4: Your First Prompt

```bash
# Test mode (doesn't actually run)
breadboard --dry-run "explain what is async programming"

# Real mode (actually runs)
breadboard "explain what is async programming"
```

## Common Questions

**Q: Which service will be used for my prompt?**

A: Run with `--dry-run` to see:
```bash
breadboard --dry-run "your prompt"
```

**Q: How do I check my usage?**

A: Run:
```bash
breadboard status
```

**Q: What if I run out of tokens?**

A: Breadboard will automatically switch to a different service that has available tokens.

**Q: Can I force a specific service?**

A: Not directly through breadboard. Use the service's CLI directly if you need that.

**Q: Where is my data stored?**

A: Database location depends on your OS:
- macOS: `~/Library/Application Support/breadboard/usage.db`
- Linux: `~/.local/share/breadboard/usage.db`
- Windows: `%LOCALAPPDATA%\breadboard\usage.db`

**Q: How do I uninstall?**

A:
```bash
# Remove the binary
sudo rm /usr/local/bin/breadboard

# Remove data (optional)
rm -rf ~/Library/Application\ Support/breadboard
rm -rf ~/.config/breadboard
```

## Example Workflow

Here's a typical day using Breadboard:

### Morning: Check Status
```bash
breadboard status
```

### During Work: Use for Various Tasks

Code generation (routes to Cursor):
```bash
breadboard "write a REST API with Express and TypeScript"
```

Debugging (routes to Cursor):
```bash
breadboard "why is my React component re-rendering infinitely?"
```

Learning (routes to Claude):
```bash
breadboard "explain how WebSockets work"
```

Code review (routes to Claude):
```bash
breadboard "review this function for security issues: [paste code]"
```

### End of Day: Check Usage
```bash
breadboard status
```

## Next Steps

Once you're comfortable with basics:

### Learn More Commands
```bash
breadboard --help           # See all commands
breadboard doctor           # Check health
breadboard stats            # View analytics
```

### Read Full Documentation
- [README.md](README.md) - Complete feature list and command reference
- [SETUP.md](SETUP.md) - Detailed installation and configuration
- [PERFORMANCE.md](PERFORMANCE.md) - Performance characteristics and optimizations

### Explore Advanced Features
```bash
# Live dashboard
breadboard dashboard

# Create templates
breadboard template save review "Review this {{language}} code: {{code}}"

# Profile management
breadboard profile create work
breadboard profile save work
```

## Getting Help

### Something Not Working?

1. **Check system health:**
   ```bash
   breadboard doctor
   ```

2. **Look for errors:**
   ```bash
   breadboard --verbose "your prompt"
   ```

3. **Check configuration:**
   ```bash
   breadboard config
   ```

### Still Stuck?

- Check [SETUP.md](SETUP.md) for troubleshooting section
- Report issue: [GitHub Issues](https://github.com/USERNAME/breadboard/issues)

## Tips for Success

1. **Always check status first**
   ```bash
   breadboard status
   ```

2. **Use dry-run to test**
   ```bash
   breadboard --dry-run "test prompt"
   ```

3. **Set up profiles for different contexts**
   ```bash
   breadboard profile create work
   breadboard profile create personal
   ```

4. **Export data monthly**
   ```bash
   breadboard export --output monthly_backup.csv
   ```

## What Breadboard Does NOT Do

- ❌ Modify your prompts
- ❌ Store your prompts permanently (only summary in history)
- ❌ Send data to external services (except direct CLI execution)
- ❌ Require internet (except for CLI execution)
- ❌ Cost money (it's free, open-source)

## Ready to Go!

You're all set. Start using Breadboard:

```bash
breadboard "write a function that reverses a string"
```

Happy routing! 🚀
