# Token Usage Format Detection

Breadboard automatically parses token usage from CLI tool outputs. This document describes supported formats.

## Supported Formats

### Pattern 1: Simple key-value
```
tokens: 1234
tokens_used: 1234
```

### Pattern 2: Input/Output breakdown
```
input_tokens: 500, output_tokens: 734
```

### Pattern 3: JSON format
```json
{
  "usage": {
    "total_tokens": 1234
  }
}
```

Or with breakdown:
```json
{
  "usage": {
    "input_tokens": 500,
    "output_tokens": 734
  }
}
```

### Pattern 4: Natural language
```
Used 1234 tokens
1234 tokens used
Consumed 1234 tokens
```

### Pattern 5: Claude-specific format
```
Tokens: input=500 output=734
input: 500 output: 734
```

## Detection Strategy

1. Searches both stdout and stderr
2. Tries all patterns in order
3. Falls back to character estimation if no match
4. Estimation: `(prompt_length + response_length) / 4`

## Making Your CLI Tools Compatible

To get accurate token tracking, ensure your CLI tool outputs token usage in one of these formats to **stdout** or **stderr**.

### Example: Claude CLI
Add to your wrapper:
```bash
echo "tokens_used: $TOKEN_COUNT" >&2
```

### Example: OpenAI/Codex
If your tool returns JSON, ensure it includes:
```json
{"usage": {"input_tokens": 500, "output_tokens": 734}}
```

### Example: Cursor
Output format like:
```
input_tokens: 500, output_tokens: 734
```

## Testing

You can test token parsing by running any breadboard command and checking the output:

```bash
breadboard "test prompt"
# Output will show: "✓ 1234 tokens used"
```

If you see unexpected token counts, your CLI tool's output format may not be recognized. Check the output manually and file an issue with the format.
