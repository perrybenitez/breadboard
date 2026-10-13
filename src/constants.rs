// P2.5: Centralized service name constants

pub const SERVICES: &[&str] = &["claude", "codex", "cursor"];

pub fn is_valid_service(name: &str) -> bool {
    SERVICES.contains(&name)
}

pub fn suggest_service(input: &str) -> Option<&'static str> {
    // Empty input should not match anything
    if input.is_empty() {
        return None;
    }

    let input_lower = input.to_lowercase();

    // Try exact match first
    for &service in SERVICES {
        if service == input_lower {
            return Some(service);
        }
    }

    // Try fuzzy matching (simple prefix/contains)
    // Only if input is at least 2 chars to avoid too broad matching
    if input_lower.len() >= 2 {
        for &service in SERVICES {
            if service.starts_with(&input_lower) || input_lower.starts_with(service) {
                return Some(service);
            }
        }
    }

    // Try Levenshtein-like simple check
    SERVICES
        .iter()
        .find(|&&service| similar(service, &input_lower))
        .copied()
}

fn similar(a: &str, b: &str) -> bool {
    // Simple similarity: if > 60% of chars match at same positions
    // AND the lengths are close (within 2 chars)
    if a.len().abs_diff(b.len()) > 2 {
        return false;
    }

    let matching = a.chars().zip(b.chars()).filter(|(x, y)| x == y).count();

    let min_len = a.len().min(b.len());
    if min_len == 0 {
        return false;
    }

    (matching as f64 / min_len as f64) > 0.6
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_service() {
        assert!(is_valid_service("claude"));
        assert!(is_valid_service("codex"));
        assert!(is_valid_service("cursor"));
        assert!(!is_valid_service("invalid"));
        assert!(!is_valid_service("gpt4"));
        assert!(!is_valid_service(""));
    }

    #[test]
    fn test_suggest_service_exact_match() {
        assert_eq!(suggest_service("claude"), Some("claude"));
        assert_eq!(suggest_service("codex"), Some("codex"));
        assert_eq!(suggest_service("cursor"), Some("cursor"));
    }

    #[test]
    fn test_suggest_service_typos() {
        assert_eq!(suggest_service("claud"), Some("claude"));
        assert_eq!(suggest_service("claue"), Some("claude"));
        assert_eq!(suggest_service("cude"), Some("codex"));
        assert_eq!(suggest_service("cursr"), Some("cursor"));
    }

    #[test]
    fn test_suggest_service_prefix() {
        assert_eq!(suggest_service("clau"), Some("claude"));
        assert_eq!(suggest_service("code"), Some("codex"));
        assert_eq!(suggest_service("curs"), Some("cursor"));
    }

    #[test]
    fn test_suggest_service_no_match() {
        let result = suggest_service("xyz");
        assert_eq!(
            result, None,
            "xyz should not match any service, got: {:?}",
            result
        );

        let result = suggest_service("gpt");
        assert_eq!(
            result, None,
            "gpt should not match any service, got: {:?}",
            result
        );

        let result = suggest_service("");
        assert_eq!(
            result, None,
            "empty string should not match any service, got: {:?}",
            result
        );
    }
}
