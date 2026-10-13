# Breadboard Documentation

Welcome to the Breadboard documentation. Choose your starting point based on your needs:

## New Users

**Start here** → [Quickstart Guide](QUICKSTART.md)

A beginner-friendly guide that assumes no prior knowledge. Walks you through:
- What Breadboard is and whether you need it
- Installation prerequisites
- Step-by-step setup
- Your first prompt
- Common questions

## Installation & Setup

**Detailed setup** → [Setup Guide](SETUP.md)

Comprehensive installation and configuration guide covering:
- Prerequisites (Rust, AI CLI tools)
- Quick start (5 minutes)
- Manual configuration options
- Troubleshooting common issues
- Configuration file locations

## Reference & Usage

**Full command reference** → [README](README.md)

Complete documentation for using Breadboard:
- All commands and options
- Configuration management
- Subscription tier setup
- Advanced features (profiles, templates, aliases)
- How routing works
- Cost tracking and analytics

## Technical Information

**Performance characteristics** → [Performance Profile](PERFORMANCE.md)

For contributors and advanced users:
- Binary size and startup time metrics
- Build optimization options
- Memory and storage usage
- Benchmark commands

**Project overview** → [Project Summary](STATUS.md)

For contributors and stakeholders:
- Implementation statistics
- Features and capabilities
- Architecture overview
- Development history

## Quick Reference

### First-Time Setup
```bash
# 1. Install
git clone https://github.com/USERNAME/breadboard.git
cd breadboard
cargo build --release
sudo cp target/release/breadboard /usr/local/bin/

# 2. Run setup wizard
breadboard setup

# 3. Verify
breadboard doctor
```

### Most Common Commands
```bash
breadboard "your prompt"              # Route and execute
breadboard status                     # Check usage
breadboard --dry-run "test prompt"    # Preview routing
breadboard doctor                     # Health check
breadboard --help                     # Full help
```

### Configuration Files
- **macOS**: `~/.config/breadboard/config.yaml` and `~/Library/Application Support/breadboard/usage.db`
- **Linux**: `~/.config/breadboard/config.yaml` and `~/.local/share/breadboard/usage.db`
- **Windows**: `%APPDATA%\\breadboard\\config.yaml` and `%LOCALAPPDATA%\\breadboard\\usage.db`

## Getting Help

### Troubleshooting
1. Run health check: `breadboard doctor`
2. Check [SETUP.md](SETUP.md) troubleshooting section
3. Enable verbose mode: `breadboard --verbose "your prompt"`
4. Report issue: [GitHub Issues](https://github.com/USERNAME/breadboard/issues)

### Resources
- [Quickstart Guide](QUICKSTART.md) - For beginners
- [Setup Guide](SETUP.md) - Detailed installation
- [README](README.md) - Complete reference
- [GitHub Issues](https://github.com/USERNAME/breadboard/issues) - Bug reports and feature requests

## Contributing

Interested in contributing? Start with:
1. [Project Summary](STATUS.md) - Understand the architecture
2. [Performance Profile](PERFORMANCE.md) - Learn about optimizations
3. Run tests: `cargo test`
4. Read the code in `src/` directory

## Document Structure

```
breadboard/
├── DOCUMENTATION.md (this file)  # Documentation hub
├── QUICKSTART.md                 # For new users
├── README.md                     # Main reference
├── SETUP.md                      # Installation guide
├── PERFORMANCE.md                # Technical details
└── STATUS.md                     # Project summary
```

---

**Questions?** Check the [Quickstart Guide](QUICKSTART.md) or [open an issue](https://github.com/USERNAME/breadboard/issues).
