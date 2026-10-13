use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceTier {
    pub name: String,
    pub tier: Tier,
    pub limits: Vec<Limit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Tier {
    // Claude
    ClaudeFree,
    ClaudePro,
    ClaudeTeam,
    ClaudeEnterprise,

    // Cursor
    CursorFree,
    CursorPro,
    CursorBusiness,

    // Codex (OpenAI)
    CodexFree,
    CodexPayAsYouGo,
    CodexScale,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Limit {
    pub window: RefreshWindow,
    pub max_tokens: i64,
    pub cost_per_1k_tokens: Option<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum RefreshWindow {
    PerMinute,
    Hourly,
    Daily,
    Monthly,
}

impl RefreshWindow {
    pub fn as_str(&self) -> &str {
        match self {
            RefreshWindow::PerMinute => "per_minute",
            RefreshWindow::Hourly => "hourly",
            RefreshWindow::Daily => "daily",
            RefreshWindow::Monthly => "monthly",
        }
    }
}

impl Tier {
    pub fn get_limits(&self) -> Vec<Limit> {
        match self {
            // Claude Pro: $20/month, ~5M tokens based on typical usage
            Tier::ClaudePro => vec![
                Limit {
                    window: RefreshWindow::Monthly,
                    max_tokens: 5_000_000,
                    cost_per_1k_tokens: Some(0.004), // ~$20 budget
                },
                Limit {
                    window: RefreshWindow::PerMinute,
                    max_tokens: 10_000,
                    cost_per_1k_tokens: None,
                },
            ],

            // Claude Team: Higher limits
            Tier::ClaudeTeam => vec![
                Limit {
                    window: RefreshWindow::Monthly,
                    max_tokens: 15_000_000,
                    cost_per_1k_tokens: Some(0.004),
                },
                Limit {
                    window: RefreshWindow::PerMinute,
                    max_tokens: 20_000,
                    cost_per_1k_tokens: None,
                },
            ],

            // Claude Free: Limited
            Tier::ClaudeFree => vec![Limit {
                window: RefreshWindow::Daily,
                max_tokens: 100_000,
                cost_per_1k_tokens: None,
            }],

            // Cursor Pro: $20/month
            Tier::CursorPro => vec![
                Limit {
                    window: RefreshWindow::Monthly,
                    max_tokens: 10_000_000,
                    cost_per_1k_tokens: Some(0.002),
                },
                Limit {
                    window: RefreshWindow::Daily,
                    max_tokens: 500_000,
                    cost_per_1k_tokens: None,
                },
            ],

            // Cursor Business: $40/month
            Tier::CursorBusiness => vec![
                Limit {
                    window: RefreshWindow::Monthly,
                    max_tokens: 20_000_000,
                    cost_per_1k_tokens: Some(0.002),
                },
                Limit {
                    window: RefreshWindow::Daily,
                    max_tokens: 1_000_000,
                    cost_per_1k_tokens: None,
                },
            ],

            // Cursor Free
            Tier::CursorFree => vec![Limit {
                window: RefreshWindow::Daily,
                max_tokens: 50_000,
                cost_per_1k_tokens: None,
            }],

            // Codex Pay-as-you-go
            Tier::CodexPayAsYouGo => vec![
                Limit {
                    window: RefreshWindow::Monthly,
                    max_tokens: 10_000_000,
                    cost_per_1k_tokens: Some(0.002),
                },
                Limit {
                    window: RefreshWindow::PerMinute,
                    max_tokens: 90_000,
                    cost_per_1k_tokens: None,
                },
            ],

            // Codex Free
            Tier::CodexFree => vec![Limit {
                window: RefreshWindow::Daily,
                max_tokens: 100_000,
                cost_per_1k_tokens: None,
            }],

            // P1.5: Add explicit limits for ClaudeEnterprise
            Tier::ClaudeEnterprise => vec![
                Limit {
                    window: RefreshWindow::Monthly,
                    max_tokens: 50_000_000, // Much higher for enterprise
                    cost_per_1k_tokens: Some(0.003), // Custom pricing
                },
                Limit {
                    window: RefreshWindow::PerMinute,
                    max_tokens: 50_000, // Higher rate limits
                    cost_per_1k_tokens: None,
                },
            ],

            // P1.5: Add explicit limits for CodexScale
            Tier::CodexScale => vec![
                Limit {
                    window: RefreshWindow::Monthly,
                    max_tokens: 50_000_000,           // High volume tier
                    cost_per_1k_tokens: Some(0.0015), // Volume discount
                },
                Limit {
                    window: RefreshWindow::PerMinute,
                    max_tokens: 150_000, // Much higher rate limits
                    cost_per_1k_tokens: None,
                },
            ],
        }
    }

    pub fn from_string(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "claude-free" => Some(Tier::ClaudeFree),
            "claude-pro" => Some(Tier::ClaudePro),
            "claude-team" => Some(Tier::ClaudeTeam),
            "claude-enterprise" => Some(Tier::ClaudeEnterprise),
            "cursor-free" => Some(Tier::CursorFree),
            "cursor-pro" => Some(Tier::CursorPro),
            "cursor-business" => Some(Tier::CursorBusiness),
            "codex-free" => Some(Tier::CodexFree),
            "codex-payg" => Some(Tier::CodexPayAsYouGo),
            "codex-scale" => Some(Tier::CodexScale),
            _ => None,
        }
    }

    /// Calculate cost in USD for given token count
    /// Uses the monthly limit's cost_per_1k_tokens if available
    pub fn calculate_cost(&self, tokens: i64) -> f64 {
        let limits = self.get_limits();

        // Find the monthly limit which typically has cost info
        for limit in limits {
            if limit.window == RefreshWindow::Monthly {
                if let Some(cost_per_1k) = limit.cost_per_1k_tokens {
                    return (tokens as f64 / 1000.0) * cost_per_1k;
                }
            }
        }

        // Fallback: check any limit with cost info
        for limit in self.get_limits() {
            if let Some(cost_per_1k) = limit.cost_per_1k_tokens {
                return (tokens as f64 / 1000.0) * cost_per_1k;
            }
        }

        0.0 // No cost info available
    }
}
