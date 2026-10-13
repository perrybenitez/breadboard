use crate::config::Config;
use crate::db::Database;
use crate::sessions::SessionContext;
use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;

// P1.1: Cache compiled regexes to avoid recompilation on every call
// Use word boundary to avoid matching "input_tokens:" or "output_tokens:"
static TOKENS_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\btokens(?:_used)?:\s*(\d+)").unwrap());
static INPUT_TOKENS_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"input_tokens:\s*(\d+)").unwrap());
static OUTPUT_TOKENS_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"output_tokens:\s*(\d+)").unwrap());
static USED_TOKENS_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?:used|consumed)\s+(\d+)\s+tokens?|(\d+)\s+tokens?\s+(?:used|consumed)")
        .unwrap()
});
static CLAUDE_FORMAT_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"input[=:]\s*(\d+).*output[=:]\s*(\d+)").unwrap());

pub struct RateLimitInfo {
    pub limits: HashMap<String, i64>, // window -> max_tokens
}

#[derive(Debug, Clone, Copy)]
pub enum TaskType {
    CodeGeneration,
    CodeRefactoring,
    CodeReview,
    Debugging,
    Documentation,
    Testing,
    Chat,
    Explanation,
}

impl TaskType {
    pub fn classify(prompt: &str) -> Self {
        let lower = prompt.to_lowercase();

        // Code generation indicators
        if lower.contains("create")
            || lower.contains("write")
            || lower.contains("implement")
            || lower.contains("build")
            || lower.contains("add function")
        {
            return TaskType::CodeGeneration;
        }

        // Refactoring indicators
        if lower.contains("refactor")
            || lower.contains("improve")
            || lower.contains("optimize")
            || lower.contains("restructure")
            || lower.contains("clean up")
        {
            return TaskType::CodeRefactoring;
        }

        // Code review indicators
        if lower.contains("review")
            || lower.contains("check")
            || lower.contains("audit")
            || lower.contains("analyze code")
        {
            return TaskType::CodeReview;
        }

        // Debugging indicators
        if lower.contains("debug")
            || lower.contains("fix")
            || lower.contains("error")
            || lower.contains("bug")
            || lower.contains("not working")
        {
            return TaskType::Debugging;
        }

        // Documentation indicators
        if lower.contains("document")
            || lower.contains("comment")
            || lower.contains("explain code")
            || lower.contains("add docs")
        {
            return TaskType::Documentation;
        }

        // Testing indicators
        if lower.contains("test")
            || lower.contains("unit test")
            || lower.contains("e2e")
            || lower.contains("integration test")
        {
            return TaskType::Testing;
        }

        // Explanation indicators
        if lower.contains("explain")
            || lower.contains("how does")
            || lower.contains("what is")
            || lower.contains("help me understand")
        {
            return TaskType::Explanation;
        }

        // Default to chat
        TaskType::Chat
    }

    pub fn description(&self) -> &str {
        match self {
            TaskType::CodeGeneration => "Code Generation",
            TaskType::CodeRefactoring => "Code Refactoring",
            TaskType::CodeReview => "Code Review",
            TaskType::Debugging => "Debugging",
            TaskType::Documentation => "Documentation",
            TaskType::Testing => "Testing",
            TaskType::Chat => "General Chat",
            TaskType::Explanation => "Explanation",
        }
    }

    pub fn config_key(&self) -> &'static str {
        match self {
            TaskType::CodeGeneration => "code_generation",
            TaskType::CodeRefactoring => "code_refactoring",
            TaskType::CodeReview => "code_review",
            TaskType::Debugging => "debugging",
            TaskType::Documentation => "documentation",
            TaskType::Testing => "testing",
            TaskType::Chat => "chat",
            TaskType::Explanation => "explanation",
        }
    }

    /// Returns ranked list of services from most to least effective
    pub fn get_effectiveness_ranking(&self) -> Vec<&'static str> {
        match self {
            TaskType::CodeGeneration => vec!["cursor", "codex", "claude"],
            TaskType::CodeRefactoring => vec!["cursor", "claude", "codex"],
            TaskType::CodeReview => vec!["claude", "cursor", "codex"],
            TaskType::Debugging => vec!["cursor", "claude", "codex"],
            TaskType::Documentation => vec!["claude", "cursor", "codex"],
            TaskType::Testing => vec!["cursor", "codex", "claude"],
            TaskType::Chat => vec!["claude", "cursor", "codex"],
            TaskType::Explanation => vec!["claude", "cursor", "codex"],
        }
    }
}

pub struct Router<'a> {
    config: &'a Config,
    db: &'a Database,
}

impl<'a> Router<'a> {
    pub fn new(config: &'a Config, db: &'a Database) -> Self {
        Router { config, db }
    }

    pub fn route(&self, task_type: TaskType, model_override: Option<&str>) -> Result<Service> {
        let ranking = task_type.get_effectiveness_ranking();
        log::debug!("Effectiveness ranking for {:?}: {:?}", task_type, ranking);

        let mut unavailable_reasons = Vec::new();

        // Try services in order of effectiveness
        for &service_name in ranking.iter() {
            log::debug!("Trying service: {}", service_name);
            match self.get_service_if_available(service_name) {
                Ok(_) => {
                    log::info!("Selected service: {}", service_name);
                    let model = model_override
                        .map(str::to_string)
                        .or_else(|| self.config.model_for(service_name, task_type.config_key()));
                    return Ok(Service::new(service_name, self.config, model));
                }
                Err(e) => {
                    log::debug!("Service {} unavailable: {}", service_name, e);
                    unavailable_reasons.push(format!("  - {}: {}", service_name, e));
                }
            }
        }

        log::error!("All services are over budget or unavailable");

        let error_msg = format!(
            "All services are over budget or unavailable\n\n\
            Reasons:\n{}\n\n\
            Actions:\n\
            - Check status: breadboard status\n\
            - Reset usage: breadboard reset\n\
            - Increase budgets: breadboard set-budget <service> <tokens>\n\
            - Configure tiers: breadboard set-tier <service> <tier>",
            unavailable_reasons.join("\n")
        );

        Err(anyhow!(error_msg))
    }

    fn get_service_if_available(&self, name: &str) -> Result<Service> {
        use crate::tiers::Tier;

        // Check monthly budget (legacy)
        let usage = self.db.get_usage(name)?;
        let budget = self.db.get_budget(name)?;

        log::debug!("Service {} monthly usage: {}/{}", name, usage, budget);

        if usage >= budget {
            log::debug!("Service {} over monthly budget", name);
            let percent = (usage as f64 / budget as f64 * 100.0) as i64;
            return Err(anyhow!(
                "over monthly budget ({}/{} tokens, {}%)",
                usage,
                budget,
                percent
            ));
        }

        // Check tier-based limits across all windows
        if let Some(tier_name) = self.db.get_tier(name)? {
            log::debug!("Service {} has tier: {}", name, tier_name);
            if let Some(tier) = Tier::from_string(&tier_name) {
                let limits = tier.get_limits();

                for limit in limits {
                    let window_usage = self.db.get_usage_window(name, limit.window.as_str())?;

                    // Use observed limit if available, otherwise use tier default
                    let max_tokens = self
                        .db
                        .get_observed_limit(name, limit.window.as_str())?
                        .unwrap_or(limit.max_tokens);

                    log::debug!(
                        "Service {} {} window: {}/{}",
                        name,
                        limit.window.as_str(),
                        window_usage,
                        max_tokens
                    );

                    if window_usage >= max_tokens {
                        log::debug!("Service {} over {} limit", name, limit.window.as_str());
                        let percent = (window_usage as f64 / max_tokens as f64 * 100.0) as i64;
                        return Err(anyhow!(
                            "over {} limit ({}/{} tokens, {}%)",
                            limit.window.as_str(),
                            window_usage,
                            max_tokens,
                            percent
                        ));
                    }
                }
            }
        } else {
            log::debug!("Service {} has no tier configured", name);
            return Err(anyhow!(
                "no tier configured (run: breadboard set-tier {} <tier>)",
                name
            ));
        }

        Ok(Service::new(name, self.config, None))
    }
}

pub struct Service {
    name: String,
    command: String,
    model: Option<String>,
}

impl Service {
    pub fn new(name: &str, config: &Config, model: Option<String>) -> Self {
        let command = match name {
            "claude" => config
                .claude_command
                .clone()
                .unwrap_or_else(|| "claude".to_string()),
            "codex" => config
                .codex_command
                .clone()
                .unwrap_or_else(|| "codex".to_string()),
            "cursor" => config
                .cursor_command
                .clone()
                .unwrap_or_else(|| "cursor".to_string()),
            _ => name.to_string(),
        };

        Service {
            name: name.to_string(),
            command,
            model,
        }
    }

    // P0.4: Validate that command is safe (not a shell command with pipes, etc.)
    fn validate_command(cmd: &str) -> bool {
        // Check that command doesn't contain shell metacharacters
        let dangerous_chars = ['|', ';', '&', '>', '<', '`', '$', '(', ')', '{', '}', '\n'];
        if cmd.chars().any(|c| dangerous_chars.contains(&c)) {
            return false;
        }

        // Check that it's a simple command name or path
        // Allow alphanumerics, dashes, underscores, dots, and forward slashes (for paths)
        cmd.chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/')
    }

    fn validate_model(model: &str) -> bool {
        !model.is_empty()
            && model.chars().all(|c| {
                c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '/' | '@')
            })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    pub fn execute(
        &self,
        prompt: &str,
        session: Option<&SessionContext>,
    ) -> Result<(i64, String, Option<RateLimitInfo>, i32)> {
        use std::process::Command;

        // P0.4: Validate command before execution
        if !Self::validate_command(&self.command) {
            log::error!("Invalid command rejected: {}", self.command);
            return Err(anyhow!(
                "Invalid or potentially dangerous command: {}",
                self.command
            ));
        }

        if let Some(model) = &self.model {
            if !Self::validate_model(model) {
                return Err(anyhow!("Invalid model identifier: {}", model));
            }
        }

        log::debug!(
            "Executing command: {} with prompt length: {}",
            self.command,
            prompt.len()
        );
        let mut command = Command::new(&self.command);

        // Provider resume syntax is intentionally isolated here. The stable
        // Breadboard session key is never passed to a provider directly.
        if self.name == "codex"
            && session
                .and_then(|s| s.provider_session_id.as_deref())
                .is_some()
        {
            command.arg("resume");
        }

        if let Some(model) = &self.model {
            command.arg("--model").arg(model);
        }

        if let Some(session) = session {
            if let Some(provider_session_id) = &session.provider_session_id {
                match self.name.as_str() {
                    "claude" => {
                        if session.is_new {
                            command.arg("--session-id").arg(provider_session_id);
                        } else {
                            command.arg("--resume").arg(provider_session_id);
                        }
                    }
                    "cursor" => {
                        command.arg("--resume").arg(provider_session_id);
                    }
                    "codex" => {
                        command.arg(provider_session_id);
                    }
                    _ => {}
                }
            }
        }

        let output = match command.arg(prompt).output() {
            Ok(output) => output,
            Err(e) => {
                // Check if command not found (os error 2)
                if e.kind() == std::io::ErrorKind::NotFound {
                    log::error!("Command '{}' not found", self.command);

                    let install_url = match self.name.as_str() {
                        "claude" => "https://anthropic.com/claude-code",
                        "cursor" => "https://cursor.sh/download",
                        "codex" => "https://platform.openai.com/",
                        _ => "the service's website",
                    };

                    return Err(anyhow!(
                        "Command '{}' not found\n\n\
                            Breadboard tried to execute: {} \"your prompt\"\n\n\
                            Fix this by:\n\
                            1. Installing {}: {}\n\
                            2. Setting custom command: breadboard config\n\
                            3. Using a different service: breadboard set-tier {} none\n\n\
                            Check which services are available: breadboard doctor",
                        self.command,
                        self.command,
                        self.name,
                        install_url,
                        self.name
                    ));
                } else {
                    // Other IO errors
                    log::error!("Failed to execute {}: {}", self.command, e);
                    return Err(anyhow!("Failed to execute {}: {}", self.command, e));
                }
            }
        };

        let exit_code = output.status.code().unwrap_or(-1);

        // P0.1: Check exit status - don't silently ignore failures
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            log::error!(
                "Command {} failed with exit code {:?}",
                self.command,
                exit_code
            );

            let error_msg = if stderr.trim().is_empty() {
                format!(
                    "{} failed with exit code {}\n\n\
                        The command executed but returned an error.\n\
                        Try running directly: {} \"your prompt\"",
                    self.command, exit_code, self.command
                )
            } else {
                format!(
                    "{} failed with exit code {}\n\n\
                        Error: {}\n\n\
                        If this persists, check: breadboard doctor",
                    self.command,
                    exit_code,
                    stderr.trim()
                )
            };

            return Err(anyhow!(error_msg));
        }

        let result = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        log::debug!("Command succeeded, parsing token usage");

        // Try to parse actual token usage from CLI output
        let tokens_used = self
            .parse_token_usage(&result, &stderr, prompt)
            .unwrap_or_else(|| {
                log::debug!("Token parsing failed, using character-based estimation");
                // Fallback: estimate from character count
                (prompt.len() + result.len()) as i64 / 4
            });

        log::info!("Tokens used: {}", tokens_used);

        // Try to parse rate limit info from stderr
        let rate_limit_info = self.parse_rate_limits(&stderr);
        if let Some(ref info) = rate_limit_info {
            log::debug!("Detected rate limits: {:?}", info.limits);
        }

        Ok((tokens_used, result, rate_limit_info, exit_code))
    }

    fn parse_token_usage(&self, stdout: &str, stderr: &str, _prompt: &str) -> Option<i64> {
        // Combine stdout and stderr for parsing
        let combined = format!("{}\n{}", stdout, stderr);

        log::debug!(
            "Attempting to parse tokens from output (length: {})",
            combined.len()
        );

        // Pattern 1: "tokens_used: 1234" or "tokens: 1234"
        if let Some(caps) = TOKENS_REGEX.captures(&combined) {
            let tokens = caps.get(1)?.as_str().parse().ok();
            log::debug!("Matched Pattern 1 (tokens_used): {:?}", tokens);
            return tokens;
        }

        // Pattern 2: "input_tokens: 500, output_tokens: 734"
        if let (Some(input), Some(output)) = (
            INPUT_TOKENS_REGEX
                .captures(&combined)
                .and_then(|c| c.get(1)?.as_str().parse::<i64>().ok()),
            OUTPUT_TOKENS_REGEX
                .captures(&combined)
                .and_then(|c| c.get(1)?.as_str().parse::<i64>().ok()),
        ) {
            log::debug!(
                "Matched Pattern 2 (input+output): {} + {} = {}",
                input,
                output,
                input + output
            );
            return Some(input + output);
        }

        // Pattern 3: JSON format like {"usage": {"total_tokens": 1234}}
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&combined) {
            if let Some(total) = json
                .get("usage")
                .and_then(|u| u.get("total_tokens"))
                .and_then(|t| t.as_i64())
            {
                return Some(total);
            }

            // Alternative: input_tokens + output_tokens
            if let (Some(input), Some(output)) = (
                json.get("usage")
                    .and_then(|u| u.get("input_tokens"))
                    .and_then(|t| t.as_i64()),
                json.get("usage")
                    .and_then(|u| u.get("output_tokens"))
                    .and_then(|t| t.as_i64()),
            ) {
                return Some(input + output);
            }
        }

        // Pattern 4: "Used 1234 tokens" or "1234 tokens used"
        if let Some(caps) = USED_TOKENS_REGEX.captures(&combined) {
            return caps.get(1).or_else(|| caps.get(2))?.as_str().parse().ok();
        }

        // Pattern 5: Claude-specific format "Tokens: input=500 output=734"
        if let Some(caps) = CLAUDE_FORMAT_REGEX.captures(&combined) {
            let input: i64 = caps.get(1)?.as_str().parse().ok()?;
            let output: i64 = caps.get(2)?.as_str().parse().ok()?;
            return Some(input + output);
        }

        None
    }

    fn parse_rate_limits(&self, stderr: &str) -> Option<RateLimitInfo> {
        let mut limits = HashMap::new();

        // Look for patterns like:
        // "rate-limit-monthly: 5000000"
        // "rate-limit-daily: 500000"
        // "rate-limit-per-minute: 10000"
        for line in stderr.lines() {
            if line.contains("rate-limit") {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() == 2 {
                    let window_part = parts[0].trim();
                    let value = parts[1].trim().parse::<i64>().ok()?;

                    let window = if window_part.contains("monthly") {
                        "monthly"
                    } else if window_part.contains("daily") {
                        "daily"
                    } else if window_part.contains("hourly") {
                        "hourly"
                    } else if window_part.contains("minute") {
                        "per_minute"
                    } else {
                        continue;
                    };

                    limits.insert(window.to_string(), value);
                }
            }
        }

        if limits.is_empty() {
            None
        } else {
            Some(RateLimitInfo { limits })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_classification_code_generation() {
        assert_eq!(
            format!("{:?}", TaskType::classify("write a function to parse JSON")),
            "CodeGeneration"
        );
        assert_eq!(
            format!("{:?}", TaskType::classify("create a new REST API")),
            "CodeGeneration"
        );
        assert_eq!(
            format!("{:?}", TaskType::classify("implement user authentication")),
            "CodeGeneration"
        );
    }

    #[test]
    fn test_task_classification_debugging() {
        assert_eq!(
            format!("{:?}", TaskType::classify("debug this error")),
            "Debugging"
        );
        assert_eq!(
            format!("{:?}", TaskType::classify("fix memory leak")),
            "Debugging"
        );
        assert_eq!(
            format!("{:?}", TaskType::classify("this is not working")),
            "Debugging"
        );
    }

    #[test]
    fn test_task_classification_refactoring() {
        assert_eq!(
            format!("{:?}", TaskType::classify("refactor this code")),
            "CodeRefactoring"
        );
        assert_eq!(
            format!("{:?}", TaskType::classify("improve performance")),
            "CodeRefactoring"
        );
        assert_eq!(
            format!("{:?}", TaskType::classify("optimize the algorithm")),
            "CodeRefactoring"
        );
    }

    #[test]
    fn test_task_classification_explanation() {
        assert_eq!(
            format!("{:?}", TaskType::classify("explain async/await")),
            "Explanation"
        );
        assert_eq!(
            format!("{:?}", TaskType::classify("what is a closure?")),
            "Explanation"
        );
        assert_eq!(
            format!("{:?}", TaskType::classify("help me understand recursion")),
            "Explanation"
        );
    }

    #[test]
    fn test_validate_command_valid() {
        assert!(Service::validate_command("claude"));
        assert!(Service::validate_command("codex"));
        assert!(Service::validate_command("cursor"));
        assert!(Service::validate_command("/usr/bin/claude"));
        assert!(Service::validate_command("./my-cli-tool"));
    }

    #[test]
    fn test_validate_command_invalid() {
        assert!(!Service::validate_command("claude | grep token"));
        assert!(!Service::validate_command("claude; rm -rf /"));
        assert!(!Service::validate_command("claude && echo foo"));
        assert!(!Service::validate_command("claude > /tmp/output"));
        assert!(!Service::validate_command("$(malicious)"));
        assert!(!Service::validate_command("`echo exploit`"));
    }

    #[test]
    fn test_validate_model() {
        assert!(Service::validate_model("gpt-5-codex"));
        assert!(Service::validate_model("claude-sonnet-4-5"));
        assert!(Service::validate_model("provider/model@latest"));
        assert!(!Service::validate_model("model with spaces"));
        assert!(!Service::validate_model("model; echo unsafe"));
        assert!(!Service::validate_model(""));
    }

    #[test]
    fn test_parse_token_usage_pattern1() {
        let config = Config::default();
        let service = Service::new("test", &config, None);

        let result = service.parse_token_usage("tokens: 1234\nSome output", "", "");
        assert_eq!(result, Some(1234));

        let result = service.parse_token_usage("tokens_used: 5678", "", "");
        assert_eq!(result, Some(5678));
    }

    #[test]
    fn test_parse_token_usage_pattern2() {
        let config = Config::default();
        let service = Service::new("test", &config, None);

        let result = service.parse_token_usage("", "input_tokens: 500, output_tokens: 734", "");
        assert_eq!(result, Some(1234));
    }

    #[test]
    fn test_parse_token_usage_pattern3_json() {
        let config = Config::default();
        let service = Service::new("test", &config, None);

        let json = r#"{"usage": {"total_tokens": 999}}"#;
        let result = service.parse_token_usage(json, "", "");
        assert_eq!(result, Some(999));

        let json = r#"{"usage": {"input_tokens": 100, "output_tokens": 200}}"#;
        let result = service.parse_token_usage(json, "", "");
        assert_eq!(result, Some(300));
    }

    #[test]
    fn test_parse_token_usage_pattern4() {
        let config = Config::default();
        let service = Service::new("test", &config, None);

        let result = service.parse_token_usage("Used 456 tokens", "", "");
        assert_eq!(result, Some(456));

        let result = service.parse_token_usage("789 tokens used", "", "");
        assert_eq!(result, Some(789));
    }

    #[test]
    fn test_parse_token_usage_pattern5() {
        let config = Config::default();
        let service = Service::new("test", &config, None);

        let result = service.parse_token_usage("Tokens: input=100 output=200", "", "");
        assert_eq!(result, Some(300));
    }

    #[test]
    fn test_parse_token_usage_no_match() {
        let config = Config::default();
        let service = Service::new("test", &config, None);

        let result = service.parse_token_usage("No token information here", "", "");
        assert_eq!(result, None);
    }
}
