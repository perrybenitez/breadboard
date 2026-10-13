# Medium-Value Features

Breadboard now includes advanced productivity and management features.

## 1. Smart Budget Alerts 🔔

### Automatic Threshold Alerts

Breadboard automatically tracks your budget usage and alerts when you hit key thresholds:

- **50% threshold**: First warning
- **75% threshold**: Second warning
- **90% threshold**: Critical warning

### How It Works

Alerts are shown after command execution:

```
✓ 1234 tokens used (45000/100000 - 45%)

🔔 Budget Alert: 50% threshold reached for claude
   Consider switching to a different service or increasing budget
```

### Alert Lifecycle

1. **First trigger**: Alert shown immediately
2. **Persistence**: Alert marked as "sent" to avoid spamming
3. **Reset**: Automatically reset on monthly billing cycle or manual reset

### Manual Reset

```bash
# Reset alerts manually (e.g., after budget increase)
breadboard reset --service claude
```

This resets both usage counters and alert flags.

### Benefits

- **Proactive warnings**: Know before you hit limits
- **Budget planning**: Adjust routing or budgets in advance
- **Cost control**: Prevent unexpected overage charges

---

## 2. Service Health Monitoring 🏥

### Track Response Times & Reliability

```bash
# View health for last 24 hours (default)
breadboard health

# View health for last week
breadboard health --hours 168
```

### Output Example

```
Service Health - Last 24 Hours

✓ claude
   Avg Response Time: 1234ms
   Success Rate: 98.5%

⚠ cursor
   Avg Response Time: 8765ms
   Success Rate: 85.2%
   3 recent failures

✗ codex
   Avg Response Time: 15432ms
   Success Rate: 72.1%
   12 recent failures
```

### Health Indicators

- **✓ Green**: >95% success rate, <5s response time
- **⚠ Yellow**: >80% success rate, <10s response time
- **✗ Red**: <80% success rate or >10s response time

### How It Works

Health metrics are automatically collected during command execution:
- Response time (milliseconds)
- Success/failure status
- Stored with timestamp for trend analysis

### Automatic Cleanup

Old health data (>7 days) is automatically cleaned up to save space.

### Use Cases

- **Identify degraded services**: Spot performance issues
- **Route optimization**: Avoid slow/unreliable services
- **SLA monitoring**: Track service quality over time

---

## 3. Configuration Profiles 👤

### Manage Multiple Configurations

Switch between different budget/tier setups instantly.

### Create a Profile

```bash
breadboard profile create work
```

### Save Current Configuration

```bash
breadboard profile save work
```

Saves:
- All service budgets
- All service tiers
- Current settings snapshot

### List Profiles

```bash
breadboard profile list
```

Output:
```
Configuration Profiles

  → work
    personal
    client-a
```

Arrow (→) indicates active profile.

### Switch Between Profiles

```bash
breadboard profile switch personal
```

Instantly loads:
- Budgets for all services
- Tier configurations
- Switches active profile

### Use Cases

**1. Work vs Personal**
```bash
# Work profile: Higher budgets, enterprise tiers
breadboard profile save work

# Personal profile: Lower budgets, pro tiers
breadboard set-budget claude 500000
breadboard set-tier claude claude-pro
breadboard profile save personal

# Switch based on context
breadboard profile switch work  # During work hours
breadboard profile switch personal  # After hours
```

**2. Client Projects**
```bash
# Separate budgets per client
breadboard profile create client-acme
breadboard set-budget claude 2000000
breadboard profile save client-acme

breadboard profile create client-globex
breadboard set-budget claude 1000000
breadboard profile save client-globex

# Switch when working on different projects
breadboard profile switch client-acme
```

**3. Team Sharing**
```bash
# Export configuration
breadboard profile save team-default

# Share the profile settings (copy budgets/tiers manually)
# Other team members can recreate with same budgets
```

---

## 4. Prompt Templates 📝

### Save Reusable Prompts

```bash
breadboard template save <name> <prompt> --description "..."
```

### Example Templates

**Code Review**
```bash
breadboard template save code-review \
  "Review this {{language}} code for security issues and best practices: {{code}}" \
  --description "Security code review template"
```

**Bug Fix**
```bash
breadboard template save debug \
  "Debug this {{language}} error: {{error_message}}. Code context: {{code}}" \
  --description "Bug debugging template"
```

**Documentation**
```bash
breadboard template save document \
  "Write documentation for this {{language}} function: {{code}}" \
  --description "Code documentation template"
```

### Variable Substitution

Templates support `{{variable}}` syntax:

```bash
# Save template with variables
breadboard template save api-test \
  "Write a {{test_framework}} test for this {{language}} API endpoint: {{endpoint}}"

# Use template with variable substitution
breadboard template use api-test \
  language=python \
  test_framework=pytest \
  endpoint="/api/users"
```

### List Templates

```bash
breadboard template list
```

Output:
```
Prompt Templates

code-review
   Security code review template
   Variables: language, code
   Preview: Review this {{language}} code for security issues and...

debug
   Bug debugging template
   Variables: language, error_message, code
   Preview: Debug this {{language}} error: {{error_message}}. Code...
```

### Use Templates

```bash
breadboard template use code-review language=python code="def foo(): pass"
```

Output:
```
→ Using template: code-review
   Security code review template

→ Task: Code generation (create, write, implement)
→ Routing to: cursor
[Response from service...]
```

### Delete Templates

```bash
breadboard template delete code-review
```

### Benefits

- **Consistency**: Standardized prompts across team
- **Efficiency**: No retyping common requests
- **Quality**: Well-crafted prompts every time
- **Variables**: Flexible with substitutions

---

## 5. Service Aliases 🏷️

### Create Custom Names for Services

```bash
breadboard alias set <alias> <service>
```

### Examples

**Common Aliases**
```bash
# Use familiar names
breadboard alias set gpt4 codex
breadboard alias set chatgpt codex
breadboard alias set sonnet claude

# Brand/version aliases
breadboard alias set ai-21 codex
breadboard alias set copilot cursor
```

**Team-Specific Names**
```bash
# Internal naming conventions
breadboard alias set fast-model cursor
breadboard alias set smart-model claude
breadboard alias set budget-model codex
```

**Migration Support**
```bash
# When migrating from another tool
breadboard alias set openai codex
breadboard alias set anthropic claude
```

### List Aliases

```bash
breadboard alias list
```

Output:
```
Service Aliases

  gpt4 → codex
  chatgpt → codex
  sonnet → claude
  fast-model → cursor
```

### Using Aliases

Aliases work transparently with set-budget, set-tier, etc.:

```bash
breadboard set-budget gpt4 2000000      # Sets budget for codex
breadboard set-tier sonnet claude-pro   # Sets tier for claude
```

**Note**: Aliases are for convenience only. The underlying service name is still used in database and routing.

### Delete Aliases

```bash
breadboard alias delete gpt4
```

### Use Cases

1. **Team onboarding**: Familiar names for new users
2. **Migration**: Gradual transition from old tool
3. **Clarity**: Descriptive names for internal use
4. **Abstraction**: Hide implementation details

---

## Quick Reference

| Feature | Commands |
|---------|----------|
| **Budget Alerts** | Automatic (no command needed) |
| | `breadboard reset --service <name>` (reset alerts) |
| **Health Monitoring** | `breadboard health` |
| | `breadboard health --hours 168` |
| **Profiles** | `breadboard profile create <name>` |
| | `breadboard profile list` |
| | `breadboard profile save <name>` |
| | `breadboard profile switch <name>` |
| **Templates** | `breadboard template save <name> <prompt>` |
| | `breadboard template list` |
| | `breadboard template use <name> var=value` |
| | `breadboard template delete <name>` |
| **Aliases** | `breadboard alias set <alias> <service>` |
| | `breadboard alias list` |
| | `breadboard alias delete <alias>` |

---

## Database Schema

### New Tables

1. **budget_alerts** - Alert tracking
   - service, alert_50_sent, alert_75_sent, alert_90_sent, last_reset

2. **health_metrics** - Performance monitoring
   - service, timestamp, response_time_ms, success

3. **profiles** - Configuration profiles
   - name, active, created_at

4. **profile_budgets** - Profile budget settings
   - profile, service, tokens

5. **profile_tiers** - Profile tier settings
   - profile, service, tier_name

6. **templates** - Prompt templates
   - name, prompt, description, variables, created_at, last_used

7. **aliases** - Service aliases
   - alias, service, created_at

---

## Benefits Summary

1. **Budget Alerts** → Never be surprised by overages
2. **Health Monitoring** → Identify problem services
3. **Profiles** → Context switching made easy
4. **Templates** → Standardized, reusable prompts
5. **Aliases** → Team-friendly naming

---

**Status**: ✅ Complete and ready to use!

Try creating your first profile or template:
```bash
breadboard profile create work
breadboard template save review "Review this code: {{code}}"
```
