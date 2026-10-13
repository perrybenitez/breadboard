# API Sync Feature

## Overview

The `breadboard sync` command queries actual usage from service APIs and compares it with local tracking to detect discrepancies.

## Usage

```bash
breadboard sync
```

## Requirements

Set environment variables for API access:

```bash
export ANTHROPIC_API_KEY="your_key_here"
export OPENAI_API_KEY="your_key_here"
```

## Supported Services

### Claude (Anthropic API)
- **Status**: Placeholder implementation
- **Note**: Anthropic doesn't currently expose a dedicated usage endpoint
- **Current behavior**: Queries the API to verify connectivity, but usage sync is not available yet
- **Future**: Will sync when Anthropic adds a usage API

### Codex (OpenAI API)
- **Status**: Full implementation
- **Endpoint**: `https://api.openai.com/v1/usage`
- **Syncs**: Total token usage from OpenAI account
- **Shows**: Comparison between API-reported usage and local tracking

### Cursor
- **Status**: Not supported
- **Reason**: Cursor has no public API
- **Current behavior**: Shows local tracking only with informational message

## Output Format

### When in sync:
```
✓ codex - In sync
   API usage: 1234567 tokens
   Local usage: 1234567 tokens
```

### When out of sync:
```
✗ codex
   API usage: 1500000 tokens
   Local usage: 1234567 tokens
   Difference: +265433 tokens
```

### When API unavailable:
```
⚠ claude
   ANTHROPIC_API_KEY not set
```

## Implementation Details

### Files
- **src/sync.rs**: Core sync logic
  - `sync_all()` - Syncs all services
  - `sync_claude()` - Claude API sync (placeholder)
  - `sync_openai()` - OpenAI API sync
  - `SyncResult` - Result struct with comparison data

- **src/main.rs**: CLI command handler
  - `Commands::Sync` enum variant
  - Display logic for sync results

### Dependencies
- `reqwest` - HTTP client for API requests
- `tokio` - Async runtime (blocking mode)

## Use Cases

1. **Verify local tracking accuracy**
   - Compare local counts with actual API usage
   - Detect if local database was reset incorrectly

2. **Detect missing usage**
   - Find usage that occurred outside of breadboard
   - Identify API calls made directly

3. **Monthly reconciliation**
   - Cross-check usage at month end
   - Ensure billing matches expectations

## Limitations

1. **Anthropic API**: No usage endpoint yet
2. **Cursor**: No public API
3. **API keys required**: Won't work without valid credentials
4. **No auto-sync**: Manual command only (not automatic)
5. **No auto-update**: Shows discrepancies but doesn't modify local data

## Future Enhancements

- Auto-sync on startup (optional)
- Auto-update local counts from API
- Historical usage trends from APIs
- Cost calculations from API billing data
- Webhook notifications for usage alerts

## Error Handling

The sync command handles various error conditions gracefully:

- Missing API keys → Warning message
- Network errors → Error message with details
- Invalid API responses → Parsing error message
- Service unavailable → Status code in error

All errors are non-fatal - the command continues syncing other services even if one fails.

## Testing

```bash
# Without API keys (should show warnings)
breadboard sync

# With API keys (should show actual usage)
export ANTHROPIC_API_KEY="sk-ant-..."
export OPENAI_API_KEY="sk-..."
breadboard sync

# Verbose mode (shows API request details)
breadboard --verbose sync
```

## Security

- API keys read from environment variables only
- Never stored in database or config files
- Transmitted over HTTPS only
- No API keys logged (even in verbose mode)

---

Generated: 2026-10-07
Status: Complete ✅
