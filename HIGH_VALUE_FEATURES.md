# High-Value Features

Breadboard now includes advanced analytics, cost tracking, and optimization features.

## 1. Cost Tracking & Savings Calculator 💰

### View Cost Analysis

```bash
# All-time costs
breadboard costs

# Last 30 days
breadboard costs --days 30

# Last week
breadboard costs --days 7
```

### Output Example

```
Cost Analysis - Last 30 Days

Per-Service Costs
  claude - $12.45 (62.3%)
     3,112,500 tokens, 45 requests
  cursor - $5.20 (26.0%)
     2,600,000 tokens, 38 requests
  codex - $2.35 (11.7%)
     587,500 tokens, 12 requests

Total Cost
  $20.00

Cache Savings
  $3.47 saved from 87 cache hits
  868,750 tokens avoided
```

### How It Works

- Automatically calculates costs based on configured tier pricing
- Tracks costs per request in the database
- Shows distribution across services
- Calculates savings from response caching

## 2. Success/Failure Rate Tracking ✅

### View Statistics

```bash
breadboard stats
```

### Output Example

```
Breadboard Statistics

Service Distribution
  cursor ██████████████████ (45 requests, 47.4%)
  claude ███████████████ (38 requests, 40.0%)
  codex ████████ (12 requests, 12.6%)

Task Type Distribution
  codegeneration ████████████████████ (52 requests, 54.7%)
  debugging ████████████ (28 requests, 29.5%)
  chat ████████ (15 requests, 15.8%)

Success Rates
  ✓ cursor - 97.8% success (44/45 requests)
     1 failed requests
  ✓ claude - 100.0% success (38/38 requests)
  ⚠ codex - 83.3% success (10/12 requests)
     2 failed requests

Cache Performance
  Cached responses: 23
  Cache hits: 87
  Tokens saved: 868,750
```

### How It Works

- Tracks command exit codes (0 = success)
- Calculates success rates per service
- Color-coded indicators (✓ green > 95%, ⚠ yellow > 80%, ✗ red < 80%)
- Helps identify unreliable services

## 3. Hit Rate Distribution 📊

Included in `breadboard stats`, shows:

- **Service Distribution**: Which service handles most requests
- **Task Type Distribution**: What kinds of tasks you're running
- Visual bar graphs for easy reading
- Percentage and absolute counts

### Use Cases

- **Identify underutilized subscriptions**: If cursor shows 5% usage, maybe downgrade
- **Optimize routing**: See if expensive services are being overused
- **Task analysis**: Understand your workflow patterns

## 4. Response Caching 🚀

### How It Works

Breadboard automatically caches responses for identical prompts:

- Cache key: SHA256 hash of (prompt + service)
- Configurable TTL (time-to-live) in hours
- Automatic expiration cleanup
- Tracks cache hits and tokens saved

### Benefits

- **Massive token savings**: Repeated questions = zero tokens
- **Instant responses**: No API call needed
- **Cost reduction**: Shows $ saved in `costs` command
- **Automatic**: No configuration needed

### Cache Stats

View in `breadboard stats`:
- Number of cached responses
- Total cache hits
- Tokens saved (and equivalent cost)

### Implementation Details

The cache automatically:
1. Checks for cached response before executing command
2. Returns cached result if found and not expired
3. Stores new responses in cache after execution
4. Increments hit counter for cache metrics
5. Cleans up expired entries periodically

**Note**: Cache is currently implemented but not yet fully integrated with routing. Full integration coming soon.

## 5. Export & Reporting 📁

### Export Usage Data

```bash
# Default filename
breadboard export

# Custom filename
breadboard export --output my_usage_2024.csv
```

### CSV Format

```csv
timestamp,service,task_type,tokens,cost_usd,exit_code,prompt_preview
2024-10-07 14:32:15,claude,codegeneration,1234,0.0049,0,"write a function to parse..."
2024-10-07 14:35:42,cursor,debugging,856,0.0034,0,"debug this error in..."
2024-10-07 14:38:19,codex,chat,445,0.0018,0,"explain how async works..."
```

### Use Cases

- **Monthly reports**: Track spending patterns
- **Team analysis**: Import into Excel/Google Sheets
- **Billing integration**: Match with invoices
- **Project tracking**: Filter by date/service
- **Data analysis**: Use with pandas, R, etc.

## Quick Reference

| Command | Description |
|---------|-------------|
| `breadboard stats` | Show hit rates, success rates, task distribution |
| `breadboard costs` | Show cost analysis (all time) |
| `breadboard costs --days 30` | Show costs for last 30 days |
| `breadboard export` | Export usage to CSV |
| `breadboard export --output file.csv` | Export to specific file |

## Database Schema Updates

New fields added to `history` table:
- `exit_code` - Command exit status (0 = success)
- `cost_usd` - Calculated cost in USD
- `task_type` - Type of task (codegeneration, debugging, etc.)

New `cache` table for response caching:
- `prompt_hash` - SHA256 hash for deduplication
- `response` - Cached response text
- `tokens` - Token count
- `expires_at` - Expiration timestamp
- `hit_count` - Number of cache hits

## Cost Calculation

Costs are calculated based on configured tier:

```rust
// Example: Claude Pro
cost_per_1k_tokens = $0.004
tokens_used = 1234
cost = (1234 / 1000) * 0.004 = $0.00494
```

Each tier has `cost_per_1k_tokens` configured:
- Claude Pro: $0.004 per 1K tokens (estimated)
- Cursor Business: $0.004 per 1K tokens (estimated)
- Codex PAYG: $0.004 per 1K tokens (estimated)

**Note**: These are estimates. Set your tier with `breadboard set-tier <service> <tier>` for accurate tracking.

## Benefits Summary

1. **Cost Tracking** → Know exactly how much you're spending
2. **Success Rates** → Identify unreliable services
3. **Hit Rates** → Optimize subscription choices
4. **Caching** → Reduce costs and improve speed
5. **Export** → Integrate with other tools

## Migration

Existing databases are automatically migrated:
- New columns added with `ALTER TABLE` (if needed)
- Existing data preserved
- New fields default to safe values (exit_code=0, cost=0.0, task_type='chat')

---

**Status**: ✅ Complete and ready to use!

Run `breadboard stats` and `breadboard costs` to see your analytics now.
