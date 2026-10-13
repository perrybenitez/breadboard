// API Sync - Query actual usage from service APIs

use crate::db::Database;
use anyhow::Result;
use serde::Deserialize;
use std::env;
use std::time::Duration;

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
    results.push(sync_claude(db)?);

    // Sync OpenAI/Codex
    log::info!("Syncing OpenAI/Codex API usage");
    results.push(sync_openai(db)?);

    // Cursor doesn't have a public API
    results.push(SyncResult {
        service: "cursor".to_string(),
        api_usage: 0,
        local_usage: db.get_usage("cursor")?,
        synced: false,
        error: Some("Cursor has no public API - using local tracking only".to_string()),
    });

    Ok(results)
}

fn sync_claude(db: &Database) -> Result<SyncResult> {
    Ok(SyncResult {
        service: "claude".to_string(),
        api_usage: 0,
        local_usage: db.get_usage("claude")?,
        synced: false,
        error: Some(
            "Anthropic does not currently expose a supported usage endpoint; using local tracking"
                .to_string(),
        ),
    })
}

fn sync_openai(db: &Database) -> Result<SyncResult> {
    let local_usage = db.get_usage("codex")?;
    let api_key = match env::var("OPENAI_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            return Ok(SyncResult {
                service: "codex".to_string(),
                api_usage: 0,
                local_usage,
                synced: false,
                error: Some("OPENAI_API_KEY not set".to_string()),
            });
        }
    };

    log::debug!("Querying OpenAI API for usage");

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;

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

                        Ok(SyncResult {
                            service: "codex".to_string(),
                            api_usage,
                            local_usage,
                            synced,
                            error: None,
                        })
                    }
                    Err(e) => Ok(SyncResult {
                        service: "codex".to_string(),
                        api_usage: 0,
                        local_usage,
                        synced: false,
                        error: Some(format!("Failed to parse response: {}", e)),
                    }),
                }
            } else {
                Ok(SyncResult {
                    service: "codex".to_string(),
                    api_usage: 0,
                    local_usage,
                    synced: false,
                    error: Some(format!("API returned status: {}", resp.status())),
                })
            }
        }
        Err(e) => Ok(SyncResult {
            service: "codex".to_string(),
            api_usage: 0,
            local_usage,
            synced: false,
            error: Some(format!("API request failed: {}", e)),
        }),
    }
}
