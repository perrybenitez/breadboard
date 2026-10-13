// API Sync - Query actual usage from service APIs

use crate::db::Database;
use anyhow::Result;
use serde::Deserialize;
use std::env;

#[derive(Debug, Deserialize)]
struct OpenAIUsageResponse {
    #[serde(default)]
    total_usage: Option<i64>,
}

pub struct SyncResult {
    pub service: String,
    pub api_usage: i64,
    pub local_usage: i64,
    pub synced: bool,
    pub error: Option<String>,
}

pub fn sync_all(db: &Database) -> Result<Vec<SyncResult>> {
    let mut results = Vec::new();

    // Sync Claude
    log::info!("Syncing Claude API usage");
    results.push(sync_claude(db));

    // Sync OpenAI/Codex
    log::info!("Syncing OpenAI/Codex API usage");
    results.push(sync_openai(db));

    // Cursor doesn't have a public API
    results.push(SyncResult {
        service: "cursor".to_string(),
        api_usage: 0,
        local_usage: db.get_usage("cursor").unwrap_or(0),
        synced: false,
        error: Some("Cursor has no public API - using local tracking only".to_string()),
    });

    Ok(results)
}

fn sync_claude(db: &Database) -> SyncResult {
    let api_key = match env::var("ANTHROPIC_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            return SyncResult {
                service: "claude".to_string(),
                api_usage: 0,
                local_usage: db.get_usage("claude").unwrap_or(0),
                synced: false,
                error: Some("ANTHROPIC_API_KEY not set".to_string()),
            };
        }
    };

    log::debug!("Querying Claude API for usage");

    let client = reqwest::blocking::Client::new();
    let response = client
        .get("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .send();

    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                // Note: Anthropic doesn't have a dedicated usage endpoint
                // Usage is returned in response headers or per-request
                // This is a placeholder for when they add it
                log::warn!("Claude API queried but usage endpoint not available yet");

                SyncResult {
                    service: "claude".to_string(),
                    api_usage: 0,
                    local_usage: db.get_usage("claude").unwrap_or(0),
                    synced: false,
                    error: Some(
                        "Anthropic doesn't expose usage API yet - using local tracking".to_string(),
                    ),
                }
            } else {
                SyncResult {
                    service: "claude".to_string(),
                    api_usage: 0,
                    local_usage: db.get_usage("claude").unwrap_or(0),
                    synced: false,
                    error: Some(format!("API returned status: {}", resp.status())),
                }
            }
        }
        Err(e) => SyncResult {
            service: "claude".to_string(),
            api_usage: 0,
            local_usage: db.get_usage("claude").unwrap_or(0),
            synced: false,
            error: Some(format!("API request failed: {}", e)),
        },
    }
}

fn sync_openai(db: &Database) -> SyncResult {
    let api_key = match env::var("OPENAI_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            return SyncResult {
                service: "codex".to_string(),
                api_usage: 0,
                local_usage: db.get_usage("codex").unwrap_or(0),
                synced: false,
                error: Some("OPENAI_API_KEY not set".to_string()),
            };
        }
    };

    log::debug!("Querying OpenAI API for usage");

    let client = reqwest::blocking::Client::new();

    // Try the usage endpoint
    let response = client
        .get("https://api.openai.com/v1/usage")
        .header("Authorization", format!("Bearer {}", api_key))
        .send();

    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<OpenAIUsageResponse>() {
                    Ok(usage_data) => {
                        let api_usage = usage_data.total_usage.unwrap_or(0);
                        let local_usage = db.get_usage("codex").unwrap_or(0);

                        // If there's a discrepancy, update local tracking
                        let synced = if api_usage != local_usage {
                            log::info!(
                                "Syncing codex: API reports {} tokens, local was {}",
                                api_usage,
                                local_usage
                            );
                            // Note: We don't auto-update because it might overwrite intentional resets
                            // User should review discrepancies
                            false
                        } else {
                            true
                        };

                        SyncResult {
                            service: "codex".to_string(),
                            api_usage,
                            local_usage,
                            synced,
                            error: None,
                        }
                    }
                    Err(e) => SyncResult {
                        service: "codex".to_string(),
                        api_usage: 0,
                        local_usage: db.get_usage("codex").unwrap_or(0),
                        synced: false,
                        error: Some(format!("Failed to parse response: {}", e)),
                    },
                }
            } else {
                SyncResult {
                    service: "codex".to_string(),
                    api_usage: 0,
                    local_usage: db.get_usage("codex").unwrap_or(0),
                    synced: false,
                    error: Some(format!("API returned status: {}", resp.status())),
                }
            }
        }
        Err(e) => SyncResult {
            service: "codex".to_string(),
            api_usage: 0,
            local_usage: db.get_usage("codex").unwrap_or(0),
            synced: false,
            error: Some(format!("API request failed: {}", e)),
        },
    }
}
