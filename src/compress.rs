//! Output compression utilities for reducing database storage while preserving
//! key information for history tracking.

#[cfg(test)]
const MAX_COMPRESSED_LENGTH: usize = 2000;
#[cfg(test)]
const CONTEXT_LINES: usize = 10;

#[cfg(test)]
pub struct CompressionResult {
    pub compressed: String,
    pub original_size: usize,
    pub compressed_size: usize,
    pub was_compressed: bool,
}

/// Compress command output for storage in history
///
/// Strategy:
/// 1. If output is small (<2KB), store as-is
/// 2. If large, keep first N lines + last N lines + summary
/// 3. Remove excessive whitespace and repetition
/// 4. Preserve error messages and important markers
#[cfg(test)]
pub fn compress_output(output: &str) -> CompressionResult {
    let original_size = output.len();

    // Small outputs don't need compression
    if original_size <= MAX_COMPRESSED_LENGTH {
        return CompressionResult {
            compressed: output.to_string(),
            original_size,
            compressed_size: original_size,
            was_compressed: false,
        };
    }

    // Split into lines
    let lines: Vec<&str> = output.lines().collect();
    let total_lines = lines.len();

    // Keep important parts
    let mut result = Vec::new();

    // Header: first N lines
    let header_end = CONTEXT_LINES.min(total_lines);
    for line in &lines[..header_end] {
        result.push(line.to_string());
    }

    // Middle indicator
    if total_lines > CONTEXT_LINES * 2 {
        result.push(format!(
            "\n... [compressed: {} lines omitted] ...\n",
            total_lines - (CONTEXT_LINES * 2)
        ));
    }

    // Footer: last N lines
    let footer_start = total_lines.saturating_sub(CONTEXT_LINES).max(header_end);
    for line in &lines[footer_start..] {
        result.push(line.to_string());
    }

    let compressed = result.join("\n");
    let compressed_size = compressed.len();

    CompressionResult {
        compressed,
        original_size,
        compressed_size,
        was_compressed: true,
    }
}

/// Compress a prompt for storage
/// Truncate very long prompts but keep enough context
pub fn compress_prompt(prompt: &str) -> String {
    const MAX_PROMPT_LENGTH: usize = 500;

    if prompt.len() <= MAX_PROMPT_LENGTH {
        prompt.to_string()
    } else {
        // P0.3: Use character boundary for UTF-8 safety
        let mut end = MAX_PROMPT_LENGTH;
        while end > 0 && !prompt.is_char_boundary(end) {
            end -= 1;
        }

        let truncated_chars = prompt[end..].chars().count();
        format!(
            "{}... [truncated {} chars]",
            &prompt[..end],
            truncated_chars
        )
    }
}

#[cfg(test)]
mod prompt_tests {
    use super::compress_prompt;

    #[test]
    fn truncation_counts_unicode_characters() {
        let prompt = format!("{}é", "a".repeat(499));
        let compressed = compress_prompt(&prompt);
        assert!(compressed.ends_with("[truncated 1 chars]"));
    }
}

/// Deduplicate consecutive identical lines (common in logs)
#[cfg(test)]
pub fn deduplicate_lines(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut result = Vec::new();
    let mut last_line: Option<&str> = None;
    let mut repeat_count = 0;

    for line in lines {
        if Some(line) == last_line {
            repeat_count += 1;
        } else {
            if repeat_count > 1 {
                result.push(format!("  ... [repeated {} times]", repeat_count));
            }
            result.push(line.to_string());
            last_line = Some(line);
            repeat_count = 1;
        }
    }

    if repeat_count > 1 {
        result.push(format!("  ... [repeated {} times]", repeat_count));
    }

    result.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_small_output() {
        let small = "Short output";
        let result = compress_output(small);

        assert!(!result.was_compressed);
        assert_eq!(result.compressed, small);
        assert_eq!(result.original_size, result.compressed_size);
    }

    #[test]
    fn test_compress_large_output() {
        // Create output that's definitely over MAX_COMPRESSED_LENGTH (2000 chars)
        let large: String = (0..200)
            .map(|i| {
                format!(
                    "This is line number {} with extra text to make it longer",
                    i
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let result = compress_output(&large);

        assert!(result.was_compressed);
        assert!(result.compressed_size < result.original_size);
        assert!(result.compressed.contains("compressed"));
    }

    #[test]
    fn test_compress_prompt() {
        let short = "Short prompt";
        assert_eq!(compress_prompt(short), short);

        let long = "a".repeat(600);
        let compressed = compress_prompt(&long);
        assert!(compressed.len() < long.len());
        assert!(compressed.contains("truncated"));
    }

    #[test]
    fn test_deduplicate_lines() {
        let text = "Line 1\nLine 2\nLine 2\nLine 2\nLine 3";
        let result = deduplicate_lines(text);

        assert!(result.contains("Line 1"));
        assert!(result.contains("Line 2"));
        assert!(result.contains("repeated 3 times"));
        assert!(result.contains("Line 3"));
    }

    #[test]
    fn test_deduplicate_no_repeats() {
        let text = "Line 1\nLine 2\nLine 3";
        let result = deduplicate_lines(text);

        assert_eq!(result, text);
    }
}
