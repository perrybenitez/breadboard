# Breadboard Unit Tests

## Overview

Breadboard includes unit tests for routing, persistence, bridge delivery, MCP
stdio behavior, and live-session turn handling.

The current suite contains 50 passing tests. It runs without provider
credentials or installed provider CLIs.

### Bridge and Live Session Tests

- Session-bound agent IDs and bounded message TTLs
- Recipient-scoped message claims and claim-token acknowledgements
- Delivery-lease expiry and retry behavior
- Durable expiry and undeliverable-message states
- Claude and Codex streamed event parsing, usage extraction, completion, and
  failure handling
- Turn-boundary bridge injection, success acknowledgment, and failed-turn
  retryability
- MCP tool schemas, message round trips, token enforcement, and notification
  response suppression
- In-memory database fixtures for isolated persistence tests

## Test Coverage

### Router Module (`src/router.rs`)

**Task Classification Tests** (5 tests)
- ✅ Code generation keywords (write, create, implement, build)
- ✅ Debugging keywords (debug, fix, error, bug)
- ✅ Refactoring keywords (refactor, improve, optimize)
- ✅ Explanation keywords (explain, what is, help understand)
- ✅ Default classification (chat)

**Command Validation Tests** (2 tests)
- ✅ Valid commands: `claude`, `codex`, `/usr/bin/claude`, `./tool`
- ✅ Invalid commands: Shell metacharacters (`|`, `;`, `&`, `>`, `$()`), backticks

**Token Parsing Tests** (7 tests)
- ✅ Pattern 1: `tokens: 1234` and `tokens_used: 5678`
- ✅ Pattern 2: `input_tokens: 500, output_tokens: 734` → 1234
- ✅ Pattern 3 (JSON): `{"usage": {"total_tokens": 999}}`
- ✅ Pattern 3 (JSON): `{"usage": {"input_tokens": 100, "output_tokens": 200}}` → 300
- ✅ Pattern 4: `Used 456 tokens` and `789 tokens used`
- ✅ Pattern 5: `Tokens: input=100 output=200` → 300
- ✅ No match fallback returns None

### Constants Module (`src/constants.rs`)

**Service Validation Tests** (6 tests)
- ✅ Valid services: claude, codex, cursor
- ✅ Invalid services: invalid, gpt4, empty string
- ✅ Exact match suggestions
- ✅ Typo correction: `claud` → `claude`, `cursr` → `cursor`
- ✅ Prefix matching: `clau` → `claude`, `code` → `codex`
- ✅ No match for unrelated inputs: xyz, gpt, empty

## Running Tests

```bash
# Run all tests
cargo test

# Run specific module
cargo test router::tests
cargo test constants::tests

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_task_classification_code_generation
```

## Test Results

```
cargo test --all-features
```
Current result:
```
50 passed; 0 failed
```

## Bug Fixes During Testing

### 1. TOKENS_REGEX Word Boundary
**Problem:** Regex was matching "input_tokens:" when looking for "tokens:"

**Fix:** Added word boundary `\b` to pattern:
```rust
static TOKENS_REGEX: Lazy<Regex> = Lazy::new(||
    Regex::new(r"\btokens(?:_used)?:\s*(\d+)").unwrap()
);
```

### 2. Case-Insensitive Token Matching
**Problem:** "Used 456 tokens" wasn't matching because regex was case-sensitive

**Fix:** Added `(?i)` flag for case-insensitive matching:
```rust
static USED_TOKENS_REGEX: Lazy<Regex> = Lazy::new(||
    Regex::new(r"(?i)(?:used|consumed)\s+(\d+)\s+tokens?|(\d+)\s+tokens?\s+(?:used|consumed)").unwrap()
);
```

### 3. Empty String Prefix Matching
**Problem:** Empty string was matching services because `str.starts_with("")` returns true

**Fix:** Added empty check and minimum length requirement:
```rust
if input.is_empty() {
    return None;
}
// Only try prefix matching if input >= 2 chars
if input_lower.len() >= 2 {
    // prefix matching logic
}
```

### 4. Length Similarity Check
**Problem:** Unrelated strings like "xyz" could match "claude" with simple char matching

**Fix:** Added length difference check:
```rust
if a.len().abs_diff(b.len()) > 2 {
    return false;
}
```

## Coverage Summary

| Area | Coverage | Status |
|------|----------|--------|
| Routing and validation | Classification, command safety, token parsing | ✅ |
| Database and bridge queue | Claims, leases, retries, expiry, acknowledgements | ✅ |
| Live sessions | Claude/Codex event parsing and turn boundaries | ✅ |
| MCP | Tool contracts and stdio request/notification behavior | ✅ |
| Provider processes | Real child-process protocol smoke tests | ⏳ |

## What's Not Tested (Future Work)

- CLI argument parsing (integration test level)
- Dashboard rendering (UI testing)
- Actual Claude/Codex command execution (requires installed providers and credentials)
- Rate limit parsing edge cases

## Continuous Integration

To add CI, create `.github/workflows/test.yml`:

```yaml
name: Tests
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - run: cargo test --all
```

---

Generated: 2026-10-12
All tests passing ✅
