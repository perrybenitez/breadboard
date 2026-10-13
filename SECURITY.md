# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

If you discover a security vulnerability in Breadboard, please help us by reporting it responsibly.

**Please DO NOT:**
- Open a public GitHub issue for security vulnerabilities
- Disclose the vulnerability publicly before it has been addressed

**Instead, please:**

1. Email security concerns to: `[your-email@domain.com]`
2. Include in your report:
   - Description of the vulnerability
   - Steps to reproduce the issue
   - Potential impact
   - Suggested fix (if you have one)
   - Your contact information for follow-up

**Our commitment:**
- We will acknowledge your email within 48 hours
- We will provide regular updates (at least every 7 days)
- We will credit you in the security advisory (unless you prefer to remain anonymous)
- We will work with you to understand and address the issue

## Security Best Practices

### API Key Management

✅ **Do:**
- Store API keys in `.env` file (automatically loaded)
- Use environment variables for API keys
- Rotate API keys regularly
- Keep `.env` file in `.gitignore` (already configured)

❌ **Don't:**
- Commit `.env` files to git
- Share your `.env` file with others
- Hardcode API keys in code or scripts

### Data Privacy

Breadboard stores data locally on your machine:

**What's stored:**
- Location: `~/Library/Application Support/breadboard/usage.db` (macOS)
- Contents: Your prompts (compressed if >500 chars), token usage, routing decisions, timestamps
- Permissions: User-only access (0600 - read/write for owner only)

**Data transmission:**
- API calls to configured services (Claude, Codex, Cursor) when you execute prompts
- Optional: API usage sync queries (only if you run `breadboard sync`)
- No telemetry or analytics sent to external servers

**Privacy considerations:**
⚠️ Your prompts may contain sensitive information:
- Source code
- API keys or credentials (avoid including these!)
- Personal data
- Business logic

**Recommendations:**
- Secure your system backups
- Don't share your database file
- Clear history periodically if needed: `breadboard reset`
- Audit your prompts before exporting: `breadboard history`

### Multi-User Systems

If running Breadboard on a shared system:

- ✅ Database has user-only permissions (0600)
- ⚠️ Config files are readable by all users (0644 by design)
- ⚠️ Don't store sensitive configs on shared systems
- ⚠️ CSV exports are created with default permissions

**On shared systems:**
```bash
# Verify database permissions
ls -la ~/Library/Application\ Support/breadboard/usage.db
# Should show: -rw------- (600)

# Set restrictive permissions on exports
umask 077  # Before exporting
breadboard export --output sensitive_data.csv
```

### Command Execution Security

Breadboard executes external CLI commands based on your configuration:

**Built-in protections:**
- ✅ Command validation before execution
- ✅ Shell metacharacters blocked (|, ;, &, >, <, etc.)
- ✅ Commands passed as arguments (not shell strings)
- ✅ No shell interpolation performed

**User responsibility:**
- Only configure trusted CLI tools
- Verify CLI tools before adding them to config
- Don't configure commands from untrusted sources

**Example secure configuration:**
```yaml
# ~/.config/breadboard/config.yaml
claude_command: claude        # ✅ Simple command name
codex_command: /usr/local/bin/codex  # ✅ Absolute path to known tool

# DON'T do this:
# claude_command: "claude | grep token"  # ❌ Will be rejected
# codex_command: "rm -rf /"              # ❌ Will be rejected
```

### CSV Export Security

When exporting usage data to CSV:

**Formula injection protection:**
- ✅ Dangerous characters (=, +, -, @) are automatically prefixed with single quote
- ✅ Prevents formula execution in Excel/Google Sheets

**Path traversal protection:**
- ✅ Export paths must be filenames only (no directories)
- ✅ Must have .csv extension
- ✅ Files created in current directory only

**Safe export examples:**
```bash
breadboard export --output usage.csv          # ✅ Safe
breadboard export --output my_usage_2024.csv  # ✅ Safe

breadboard export --output ../etc/passwd.csv  # ❌ Rejected (path traversal)
breadboard export --output usage.txt          # ❌ Rejected (wrong extension)
```

### Clearing Sensitive Data

**Reset usage counters** (keeps configuration and history):
```bash
breadboard reset
```

**Clear specific service:**
```bash
breadboard reset --service claude
```

**Delete all data** (complete removal):
```bash
# macOS
rm ~/Library/Application\ Support/breadboard/usage.db
rm ~/.config/breadboard/config.yaml

# Linux
rm ~/.local/share/breadboard/usage.db
rm ~/.config/breadboard/config.yaml

# Windows
del %LOCALAPPDATA%\breadboard\usage.db
del %APPDATA%\breadboard\config.yaml
```

## Known Limitations

### Security Trade-offs

1. **API Keys in Environment**
   - Stored in `.env` file (not encrypted at rest)
   - Loaded into process memory during execution
   - Use OS-level encryption (FileVault, BitLocker) for at-rest protection

2. **Prompt History Persistence**
   - All prompts stored until manually cleared
   - Compressed but not encrypted
   - Contains full text of your requests

3. **CSV Exports**
   - May contain sensitive data from prompts
   - Created with default file permissions
   - Review before sharing

4. **No Built-in Encryption**
   - Database stored in plaintext SQLite
   - Relies on OS filesystem encryption
   - Consider full-disk encryption for sensitive data

## Security Updates

### How to Stay Secure

1. **Keep Breadboard Updated**
   ```bash
   cd breadboard
   git pull
   cargo build --release
   ```

2. **Check for Security Advisories**
   - Watch GitHub repository for security releases
   - Subscribe to repository notifications
   - Check CHANGELOG.md for security fixes

3. **Run Security Audit** (for developers):
   ```bash
   cargo install cargo-audit
   cargo audit
   ```

4. **Monitor Dependencies**
   - GitHub Dependabot enabled (automatic PR for vulnerable dependencies)
   - Review and merge security updates promptly

### Verified Security Features

✅ **No unsafe Rust code** - All code is memory-safe
✅ **Parameterized SQL queries** - No SQL injection vulnerabilities
✅ **Command validation** - Shell injection protected
✅ **Input sanitization** - CSV formula injection prevented
✅ **Path validation** - Directory traversal prevented
✅ **No hardcoded secrets** - All credentials via environment
✅ **Restrictive file permissions** - Database user-only (0600)

## Questions?

For security questions or concerns:
- Security issues: `[your-email@domain.com]`
- General questions: [GitHub Discussions](https://github.com/USERNAME/breadboard/discussions)
- Bug reports: [GitHub Issues](https://github.com/USERNAME/breadboard/issues)

---

**Last Updated**: 2026-10-09
**Security Audit Date**: 2026-10-09
**Audit Status**: Passed (Grade: A-)
