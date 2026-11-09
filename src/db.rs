use anyhow::{bail, Result};
use chrono::{Datelike, Timelike, Utc};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::path::PathBuf;
use std::time::Duration;

pub struct Database {
    conn: Connection,
}

pub struct HistoryEntry {
    pub timestamp: String,
    pub service: String,
    pub prompt_preview: String,
    pub tokens: i64,
    pub exit_code: i32,
    pub cost_usd: f64,
    pub task_type: String,
}

pub struct CostSummary {
    pub service: String,
    pub total_cost: f64,
    pub total_tokens: i64,
    pub request_count: i64,
}

pub struct SuccessRate {
    pub service: String,
    pub total_requests: i64,
    pub successful: i64,
    pub failed: i64,
    pub success_rate: f64,
}

pub struct HitRate {
    pub service: String,
    pub count: i64,
    pub percentage: f64,
}

pub struct TaskDistribution {
    pub task_type: String,
    pub count: i64,
    pub percentage: f64,
}

pub struct HealthMetric {
    pub service: String,
    pub avg_response_time_ms: i64,
    pub success_rate: f64,
    pub recent_failures: i64,
}

pub struct Template {
    pub name: String,
    pub prompt: String,
    pub description: Option<String>,
    pub variables: Vec<String>,
}

pub struct Profile {
    pub name: String,
    pub active: bool,
}

pub struct SessionRecord {
    pub session_key: String,
    pub session_name: String,
    pub service: String,
    pub model_id: String,
    pub project_root: String,
    pub provider_session_id: Option<String>,
    pub status: String,
    pub created_at: String,
    pub last_used_at: String,
}

pub struct AgentRecord {
    pub agent_id: String,
    pub display_name: String,
    pub provider: String,
    pub model_id: String,
    pub session_key: Option<String>,
    pub capabilities: Vec<String>,
    pub status: String,
    pub last_seen_at: String,
}

pub struct BridgeMessage {
    pub message_id: String,
    pub from_agent: String,
    pub to_agent: String,
    pub task_id: Option<String>,
    pub message_type: String,
    pub payload: String,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub delivered_at: Option<String>,
    pub acked_at: Option<String>,
    pub attempts: i64,
    pub state: String,
    pub claimed_by: Option<String>,
    pub claim_token: Option<String>,
    pub claim_expires_at: Option<String>,
}

fn bridge_message_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<BridgeMessage> {
    Ok(BridgeMessage {
        message_id: row.get(0)?,
        from_agent: row.get(1)?,
        to_agent: row.get(2)?,
        task_id: row.get(3)?,
        message_type: row.get(4)?,
        payload: row.get(5)?,
        created_at: row.get(6)?,
        expires_at: row.get(7)?,
        delivered_at: row.get(8)?,
        acked_at: row.get(9)?,
        attempts: row.get(10)?,
        state: row.get(11)?,
        claimed_by: row.get(12)?,
        claim_token: row.get(13)?,
        claim_expires_at: row.get(14)?,
    })
}

impl Database {
    pub fn new() -> Result<Self> {
        let db_path = Self::get_db_path()?;
        let mut conn = Connection::open(db_path)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.set_transaction_behavior(TransactionBehavior::Immediate);

        conn.execute(
            "CREATE TABLE IF NOT EXISTS usage (
                id INTEGER PRIMARY KEY,
                service TEXT NOT NULL,
                month TEXT NOT NULL,
                tokens INTEGER NOT NULL DEFAULT 0,
                UNIQUE(service, month)
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS budgets (
                service TEXT PRIMARY KEY,
                tokens INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                service TEXT NOT NULL,
                prompt TEXT NOT NULL,
                tokens INTEGER NOT NULL,
                exit_code INTEGER DEFAULT 0,
                cost_usd REAL DEFAULT 0.0,
                task_type TEXT DEFAULT 'chat'
            )",
            [],
        )?;

        // Add new columns if they don't exist (for existing databases)
        let _ = conn.execute(
            "ALTER TABLE history ADD COLUMN exit_code INTEGER DEFAULT 0",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE history ADD COLUMN cost_usd REAL DEFAULT 0.0",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE history ADD COLUMN task_type TEXT DEFAULT 'chat'",
            [],
        );

        // Track usage per time window (minute, hour, day, month)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS usage_windows (
                id INTEGER PRIMARY KEY,
                service TEXT NOT NULL,
                window TEXT NOT NULL,
                window_start DATETIME NOT NULL,
                tokens INTEGER NOT NULL DEFAULT 0,
                UNIQUE(service, window, window_start)
            )",
            [],
        )?;

        // Store observed rate limits from API responses
        conn.execute(
            "CREATE TABLE IF NOT EXISTS rate_limits (
                service TEXT NOT NULL,
                window TEXT NOT NULL,
                max_tokens INTEGER NOT NULL,
                last_observed DATETIME DEFAULT CURRENT_TIMESTAMP,
                source TEXT NOT NULL,
                PRIMARY KEY(service, window)
            )",
            [],
        )?;

        // Store tier configuration
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tiers (
                service TEXT PRIMARY KEY,
                tier_name TEXT NOT NULL,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        // Response cache for identical prompts
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cache (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                prompt_hash TEXT NOT NULL,
                service TEXT NOT NULL,
                prompt TEXT NOT NULL,
                response TEXT NOT NULL,
                tokens INTEGER NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                expires_at DATETIME NOT NULL,
                hit_count INTEGER DEFAULT 0,
                UNIQUE(prompt_hash, service)
            )",
            [],
        )?;

        // Budget alerts configuration
        conn.execute(
            "CREATE TABLE IF NOT EXISTS budget_alerts (
                service TEXT PRIMARY KEY,
                alert_50_sent BOOLEAN DEFAULT 0,
                alert_75_sent BOOLEAN DEFAULT 0,
                alert_90_sent BOOLEAN DEFAULT 0,
                last_reset DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        // Service health tracking (response times)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS health_metrics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                service TEXT NOT NULL,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                response_time_ms INTEGER NOT NULL,
                success BOOLEAN NOT NULL
            )",
            [],
        )?;

        // Configuration profiles
        conn.execute(
            "CREATE TABLE IF NOT EXISTS profiles (
                name TEXT PRIMARY KEY,
                active BOOLEAN DEFAULT 0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS profile_budgets (
                profile TEXT NOT NULL,
                service TEXT NOT NULL,
                tokens INTEGER NOT NULL,
                PRIMARY KEY(profile, service),
                FOREIGN KEY(profile) REFERENCES profiles(name)
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS profile_tiers (
                profile TEXT NOT NULL,
                service TEXT NOT NULL,
                tier_name TEXT NOT NULL,
                PRIMARY KEY(profile, service),
                FOREIGN KEY(profile) REFERENCES profiles(name)
            )",
            [],
        )?;

        // Prompt templates
        conn.execute(
            "CREATE TABLE IF NOT EXISTS templates (
                name TEXT PRIMARY KEY,
                prompt TEXT NOT NULL,
                description TEXT,
                variables TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                last_used DATETIME
            )",
            [],
        )?;

        // Service aliases
        conn.execute(
            "CREATE TABLE IF NOT EXISTS aliases (
                alias TEXT PRIMARY KEY,
                service TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        // Breadboard session registry. Provider session IDs are optional because
        // providers expose different persistence and resume capabilities.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_key TEXT UNIQUE NOT NULL,
                session_name TEXT NOT NULL,
                service TEXT NOT NULL,
                model_id TEXT NOT NULL,
                project_root TEXT NOT NULL,
                provider_session_id TEXT,
                status TEXT NOT NULL DEFAULT 'active',
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                last_used_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS bridge_agents (
                agent_id TEXT PRIMARY KEY,
                display_name TEXT NOT NULL,
                provider TEXT NOT NULL,
                model_id TEXT NOT NULL,
                session_key TEXT,
                capabilities TEXT NOT NULL DEFAULT '[]',
                status TEXT NOT NULL DEFAULT 'online',
                last_seen_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS bridge_messages (
                message_id TEXT PRIMARY KEY,
                from_agent TEXT NOT NULL,
                to_agent TEXT NOT NULL,
                task_id TEXT,
                message_type TEXT NOT NULL,
                payload TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                expires_at DATETIME,
                delivered_at DATETIME,
                acked_at DATETIME,
                attempts INTEGER NOT NULL DEFAULT 0,
                state TEXT NOT NULL DEFAULT 'pending',
                claimed_by TEXT,
                claim_token TEXT,
                claim_expires_at DATETIME
            )",
            [],
        )?;

        // Bridge schema migrations for databases created before delivery leases
        // and durable terminal states were introduced.
        add_bridge_column_if_missing(
            &conn,
            "ALTER TABLE bridge_messages ADD COLUMN state TEXT NOT NULL DEFAULT 'pending'",
        )?;
        add_bridge_column_if_missing(
            &conn,
            "ALTER TABLE bridge_messages ADD COLUMN claimed_by TEXT",
        )?;
        add_bridge_column_if_missing(
            &conn,
            "ALTER TABLE bridge_messages ADD COLUMN claim_token TEXT",
        )?;
        add_bridge_column_if_missing(
            &conn,
            "ALTER TABLE bridge_messages ADD COLUMN claim_expires_at DATETIME",
        )?;
        conn.execute(
            "UPDATE bridge_messages SET state = 'acked'
             WHERE acked_at IS NOT NULL AND state = 'pending'",
            [],
        )?;

        // P2.3: Create indexes for performance
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_history_timestamp ON history(timestamp DESC)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_history_service ON history(service)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_cache_prompt_hash ON cache(prompt_hash)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_cache_expires ON cache(expires_at)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_health_metrics_service ON health_metrics(service, timestamp DESC)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_templates_last_used ON templates(last_used DESC)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_sessions_last_used ON sessions(last_used_at DESC)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bridge_agents_status ON bridge_agents(status, last_seen_at DESC)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bridge_messages_inbox
             ON bridge_messages(to_agent, state, claim_expires_at, created_at)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bridge_messages_delivery
             ON bridge_messages(to_agent, state, claim_expires_at, created_at)",
            [],
        )?;

        // Note: usage_windows(service, window, window_start) already has UNIQUE constraint
        // which provides an implicit index, so no additional index needed

        // Initialize default budgets if not set
        for service in crate::constants::SERVICES {
            conn.execute(
                "INSERT OR IGNORE INTO budgets (service, tokens) VALUES (?1, ?2)",
                params![service, 1_000_000],
            )?;
        }

        let db = Database { conn };

        // Set restrictive permissions on database file (user-only access)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let db_path = Self::get_db_path()?;
            let mut perms = std::fs::metadata(&db_path)?.permissions();
            perms.set_mode(0o600); // rw------- (user read/write only)
            std::fs::set_permissions(&db_path, perms)?;
        }

        // P1.3: Clean up old window data on initialization
        db.cleanup_old_windows()?;
        db.cleanup_expired_bridge_messages()?;

        Ok(db)
    }

    fn get_db_path() -> Result<PathBuf> {
        let mut path = dirs::data_local_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find local data directory"))?;
        path.push("breadboard");
        std::fs::create_dir_all(&path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&path)?.permissions();
            permissions.set_mode(0o700);
            std::fs::set_permissions(&path, permissions)?;
        }
        path.push("usage.db");
        Ok(path)
    }

    #[cfg(test)]
    pub(crate) fn in_memory_for_tests() -> Result<Self> {
        let mut conn = Connection::open_in_memory()?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.set_transaction_behavior(TransactionBehavior::Immediate);
        conn.execute_batch(
            "CREATE TABLE budgets (service TEXT PRIMARY KEY, tokens INTEGER NOT NULL);
             CREATE TABLE usage (
                 id INTEGER PRIMARY KEY,
                 service TEXT NOT NULL,
                 month TEXT NOT NULL,
                 tokens INTEGER NOT NULL DEFAULT 0,
                 UNIQUE(service, month)
             );
             CREATE TABLE history (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                 service TEXT NOT NULL,
                 prompt TEXT NOT NULL,
                 tokens INTEGER NOT NULL,
                 exit_code INTEGER DEFAULT 0,
                 cost_usd REAL DEFAULT 0.0,
                 task_type TEXT DEFAULT 'chat'
             );
             CREATE TABLE usage_windows (
                 id INTEGER PRIMARY KEY,
                 service TEXT NOT NULL,
                 window TEXT NOT NULL,
                 window_start DATETIME NOT NULL,
                 tokens INTEGER NOT NULL DEFAULT 0,
                 UNIQUE(service, window, window_start)
             );
             CREATE TABLE bridge_agents (
                 agent_id TEXT PRIMARY KEY,
                 display_name TEXT NOT NULL,
                 provider TEXT NOT NULL,
                 model_id TEXT NOT NULL,
                 session_key TEXT,
                 capabilities TEXT NOT NULL DEFAULT '[]',
                 status TEXT NOT NULL DEFAULT 'online',
                 last_seen_at DATETIME DEFAULT CURRENT_TIMESTAMP
             );
             CREATE TABLE bridge_messages (
                 message_id TEXT PRIMARY KEY,
                 from_agent TEXT NOT NULL,
                 to_agent TEXT NOT NULL,
                 task_id TEXT,
                 message_type TEXT NOT NULL,
                 payload TEXT NOT NULL,
                 created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                 expires_at DATETIME,
                 delivered_at DATETIME,
                 acked_at DATETIME,
                 attempts INTEGER NOT NULL DEFAULT 0,
                 state TEXT NOT NULL DEFAULT 'pending',
                 claimed_by TEXT,
                 claim_token TEXT,
                 claim_expires_at DATETIME
             );",
        )?;
        Ok(Database { conn })
    }

    fn current_month() -> String {
        let now = Utc::now();
        format!("{}-{:02}", now.year(), now.month())
    }

    pub fn get_usage(&self, service: &str) -> Result<i64> {
        let month = Self::current_month();
        // P1.4: Properly handle database errors - only return 0 for missing rows
        match self.conn.query_row(
            "SELECT tokens FROM usage WHERE service = ?1 AND month = ?2",
            params![service, month],
            |row| row.get(0),
        ) {
            Ok(usage) => Ok(usage),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(0),
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_budget(&self, service: &str) -> Result<i64> {
        // P1.4: Properly handle database errors - only return default for missing rows
        match self.conn.query_row(
            "SELECT tokens FROM budgets WHERE service = ?1",
            params![service],
            |row| row.get(0),
        ) {
            Ok(budget) => Ok(budget),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(1_000_000),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set_budget(&self, service: &str, tokens: i64) -> Result<()> {
        if tokens <= 0 {
            bail!("budget must be greater than zero");
        }
        self.conn.execute(
            "INSERT OR REPLACE INTO budgets (service, tokens) VALUES (?1, ?2)",
            params![service, tokens],
        )?;
        Ok(())
    }

    pub fn record_usage(
        &self,
        service: &str,
        tokens: i64,
        prompt: &str,
        exit_code: i32,
        cost_usd: f64,
        task_type: &str,
    ) -> Result<()> {
        let month = Self::current_month();

        // P0.3: Use transaction to ensure atomic updates across all tables
        self.conn.execute("BEGIN IMMEDIATE", [])?;

        let result = (|| -> Result<()> {
            // Update monthly usage
            self.conn.execute(
                "INSERT INTO usage (service, month, tokens) VALUES (?1, ?2, ?3)
                 ON CONFLICT(service, month) DO UPDATE SET tokens = tokens + ?3",
                params![service, month, tokens],
            )?;

            // Record in history with cost and success tracking
            self.conn.execute(
                "INSERT INTO history (service, prompt, tokens, exit_code, cost_usd, task_type)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![service, prompt, tokens, exit_code, cost_usd, task_type],
            )?;

            // Track usage across all windows
            for window in &["per_minute", "hourly", "daily", "monthly"] {
                self.record_usage_window(service, tokens, window)?;
            }

            Ok(())
        })();

        match result {
            Ok(_) => {
                self.conn.execute("COMMIT", [])?;
                Ok(())
            }
            Err(e) => {
                self.conn.execute("ROLLBACK", [])?;
                Err(e)
            }
        }
    }

    pub fn reset_usage(&self, service: &str) -> Result<()> {
        let month = Self::current_month();
        self.conn.execute(
            "DELETE FROM usage WHERE service = ?1 AND month = ?2",
            params![service, month],
        )?;
        Ok(())
    }

    pub fn reset_all_usage(&self) -> Result<()> {
        let month = Self::current_month();
        self.conn
            .execute("DELETE FROM usage WHERE month = ?1", params![month])?;
        Ok(())
    }

    pub fn get_history(&self, limit: usize) -> Result<Vec<HistoryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT timestamp, service, prompt, tokens, exit_code, cost_usd, task_type FROM history
             ORDER BY timestamp DESC LIMIT ?1",
        )?;

        let entries = stmt
            .query_map(params![limit as i64], |row| {
                let timestamp: String = row.get(0)?;
                let service: String = row.get(1)?;
                let prompt: String = row.get(2)?;
                let tokens: i64 = row.get(3)?;
                let exit_code: i32 = row.get(4).unwrap_or(0);
                let cost_usd: f64 = row.get(5).unwrap_or(0.0);
                let task_type: String = row.get(6).unwrap_or_else(|_| "chat".to_string());

                // P1.2: Use character-aware truncation to avoid panicking on multi-byte UTF-8
                let prompt_preview = if prompt.chars().count() > 50 {
                    format!("{}...", prompt.chars().take(47).collect::<String>())
                } else {
                    prompt
                };

                Ok(HistoryEntry {
                    timestamp,
                    service,
                    prompt_preview,
                    tokens,
                    exit_code,
                    cost_usd,
                    task_type,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(entries)
    }

    pub fn create_session(
        &self,
        session_key: &str,
        session_name: &str,
        service: &str,
        model_id: &str,
        project_root: &str,
        provider_session_id: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sessions
             (session_key, session_name, service, model_id, project_root, provider_session_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                session_key,
                session_name,
                service,
                model_id,
                project_root,
                provider_session_id
            ],
        )?;
        Ok(())
    }

    pub fn get_session(&self, session_key: &str) -> Result<Option<SessionRecord>> {
        let result = self
            .conn
            .query_row(
                "SELECT session_key, session_name, service, model_id, project_root,
                        provider_session_id, status, created_at, last_used_at
                 FROM sessions WHERE session_key = ?1",
                params![session_key],
                |row| {
                    Ok(SessionRecord {
                        session_key: row.get(0)?,
                        session_name: row.get(1)?,
                        service: row.get(2)?,
                        model_id: row.get(3)?,
                        project_root: row.get(4)?,
                        provider_session_id: row.get(5)?,
                        status: row.get(6)?,
                        created_at: row.get(7)?,
                        last_used_at: row.get(8)?,
                    })
                },
            )
            .optional()?;
        Ok(result)
    }

    pub fn touch_session(&self, session_key: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET last_used_at = CURRENT_TIMESTAMP WHERE session_key = ?1",
            params![session_key],
        )?;
        Ok(())
    }

    pub fn set_provider_session_id(
        &self,
        session_key: &str,
        provider_session_id: &str,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions
             SET provider_session_id = ?2, last_used_at = CURRENT_TIMESTAMP
             WHERE session_key = ?1",
            params![session_key, provider_session_id],
        )?;
        Ok(())
    }

    pub fn list_sessions(&self) -> Result<Vec<SessionRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT session_key, session_name, service, model_id, project_root,
                    provider_session_id, status, created_at, last_used_at
             FROM sessions ORDER BY last_used_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(SessionRecord {
                session_key: row.get(0)?,
                session_name: row.get(1)?,
                service: row.get(2)?,
                model_id: row.get(3)?,
                project_root: row.get(4)?,
                provider_session_id: row.get(5)?,
                status: row.get(6)?,
                created_at: row.get(7)?,
                last_used_at: row.get(8)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn delete_session(&self, session_key: &str) -> Result<bool> {
        let deleted = self.conn.execute(
            "DELETE FROM sessions WHERE session_key = ?1",
            params![session_key],
        )?;
        Ok(deleted > 0)
    }

    pub fn register_agent(
        &self,
        agent_id: &str,
        display_name: &str,
        provider: &str,
        model_id: &str,
        session_key: Option<&str>,
        capabilities: &[String],
    ) -> Result<()> {
        let capabilities = serde_json::to_string(capabilities)?;
        self.conn.execute(
            "INSERT INTO bridge_agents
             (agent_id, display_name, provider, model_id, session_key, capabilities, status, last_seen_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'online', CURRENT_TIMESTAMP)
             ON CONFLICT(agent_id) DO UPDATE SET
                display_name = ?2,
                provider = ?3,
                model_id = ?4,
                session_key = ?5,
                capabilities = ?6,
                status = 'online',
                last_seen_at = CURRENT_TIMESTAMP",
            params![agent_id, display_name, provider, model_id, session_key, capabilities],
        )?;
        Ok(())
    }

    pub fn touch_agent(&self, agent_id: &str) -> Result<bool> {
        let updated = self.conn.execute(
            "UPDATE bridge_agents
             SET status = 'online', last_seen_at = CURRENT_TIMESTAMP
             WHERE agent_id = ?1",
            params![agent_id],
        )?;
        Ok(updated > 0)
    }

    pub fn mark_agent_offline(&self, agent_id: &str) -> Result<bool> {
        let updated = self.conn.execute(
            "UPDATE bridge_agents SET status = 'offline'
             WHERE agent_id = ?1",
            params![agent_id],
        )?;
        Ok(updated > 0)
    }

    pub fn refresh_agent_status(&self) -> Result<()> {
        self.conn.execute(
            "UPDATE bridge_agents SET status = 'offline'
             WHERE status = 'online'
               AND last_seen_at <= datetime('now', '-15 minutes')",
            [],
        )?;
        Ok(())
    }

    pub fn list_agents(&self) -> Result<Vec<AgentRecord>> {
        self.refresh_agent_status()?;
        let mut stmt = self.conn.prepare(
            "SELECT agent_id, display_name, provider, model_id, session_key,
                    capabilities, status, last_seen_at
             FROM bridge_agents ORDER BY last_seen_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let capabilities_json: String = row.get(5)?;
            let capabilities = serde_json::from_str(&capabilities_json).unwrap_or_default();
            Ok(AgentRecord {
                agent_id: row.get(0)?,
                display_name: row.get(1)?,
                provider: row.get(2)?,
                model_id: row.get(3)?,
                session_key: row.get(4)?,
                capabilities,
                status: row.get(6)?,
                last_seen_at: row.get(7)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn agent_exists(&self, agent_id: &str) -> Result<bool> {
        let exists: i64 = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM bridge_agents WHERE agent_id = ?1)",
            params![agent_id],
            |row| row.get(0),
        )?;
        Ok(exists != 0)
    }

    pub fn unregister_agent(&self, agent_id: &str) -> Result<bool> {
        self.conn.execute(
            "UPDATE bridge_messages
             SET state = 'undeliverable'
             WHERE to_agent = ?1 AND acked_at IS NULL
               AND state IN ('pending', 'claimed')",
            params![agent_id],
        )?;
        let deleted = self.conn.execute(
            "DELETE FROM bridge_agents WHERE agent_id = ?1",
            params![agent_id],
        )?;
        Ok(deleted > 0)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn send_bridge_message(
        &self,
        message_id: &str,
        from_agent: &str,
        to_agent: &str,
        task_id: Option<&str>,
        message_type: &str,
        payload: &str,
        expires_at: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO bridge_messages
             (message_id, from_agent, to_agent, task_id, message_type, payload, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                message_id,
                from_agent,
                to_agent,
                task_id,
                message_type,
                payload,
                expires_at
            ],
        )?;
        Ok(())
    }

    pub fn get_bridge_inbox(
        &self,
        agent_id: &str,
        include_acked: bool,
    ) -> Result<Vec<BridgeMessage>> {
        self.cleanup_expired_bridge_messages()?;

        if include_acked {
            return self.query_bridge_messages(
                "WHERE to_agent = ?1 ORDER BY created_at ASC",
                params![agent_id],
            );
        }

        // IMMEDIATE prevents two Breadboard processes from claiming the same
        // pending message between the select and update steps.
        let tx = self.conn.unchecked_transaction()?;
        let mut stmt = tx.prepare(
            "SELECT message_id
             FROM bridge_messages
             WHERE to_agent = ?1 AND state = 'pending' AND acked_at IS NULL
               AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)
               AND (claim_expires_at IS NULL OR claim_expires_at <= CURRENT_TIMESTAMP)
             ORDER BY created_at ASC LIMIT 50",
        )?;
        let ids = stmt
            .query_map(params![agent_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);

        let mut messages = Vec::with_capacity(ids.len());
        for message_id in ids {
            let updated = tx.execute(
                "UPDATE bridge_messages
                 SET state = 'claimed', claimed_by = ?2,
                     claim_token = 'cl_' || lower(hex(randomblob(16))),
                     claim_expires_at = datetime('now', '+5 minutes'),
                     delivered_at = COALESCE(delivered_at, CURRENT_TIMESTAMP),
                     attempts = attempts + 1
                 WHERE message_id = ?1 AND to_agent = ?2 AND state = 'pending'
                   AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)",
                params![message_id, agent_id],
            )?;
            if updated == 0 {
                // A message can expire between the claim query and this
                // update. Do not return an unclaimed or expired row to the
                // caller as if it were delivered.
                continue;
            }

            let message = tx.query_row(
                "SELECT message_id, from_agent, to_agent, task_id, message_type, payload,
                        created_at, expires_at, delivered_at, acked_at, attempts,
                        state, claimed_by, claim_token, claim_expires_at
                 FROM bridge_messages WHERE message_id = ?1",
                params![message_id],
                bridge_message_from_row,
            )?;
            messages.push(message);
        }
        tx.commit()?;
        Ok(messages)
    }

    pub fn ack_bridge_message(
        &self,
        agent_id: &str,
        message_id: &str,
        claim_token: &str,
    ) -> Result<bool> {
        let updated = self.conn.execute(
            "UPDATE bridge_messages SET state = 'acked', acked_at = CURRENT_TIMESTAMP,
                    claimed_by = NULL, claim_token = NULL, claim_expires_at = NULL
             WHERE message_id = ?1 AND to_agent = ?2 AND claim_token = ?3
               AND state = 'claimed' AND acked_at IS NULL
               AND claim_expires_at > CURRENT_TIMESTAMP",
            params![message_id, agent_id, claim_token],
        )?;
        Ok(updated > 0)
    }

    pub fn query_bridge_messages(
        &self,
        condition: &str,
        condition_params: impl rusqlite::Params,
    ) -> Result<Vec<BridgeMessage>> {
        let query = format!(
            "SELECT message_id, from_agent, to_agent, task_id, message_type, payload,
                    created_at, expires_at, delivered_at, acked_at, attempts,
                    state, claimed_by, claim_token, claim_expires_at
             FROM bridge_messages {}",
            condition
        );
        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map(condition_params, bridge_message_from_row)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn bridge_status(&self) -> Result<(i64, i64, i64)> {
        self.refresh_agent_status()?;
        self.cleanup_expired_bridge_messages()?;
        let agents: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM bridge_agents WHERE status = 'online'",
            [],
            |row| row.get(0),
        )?;
        let pending: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM bridge_messages
             WHERE state IN ('pending', 'claimed') AND acked_at IS NULL",
            [],
            |row| row.get(0),
        )?;
        let total: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM bridge_messages", [], |row| row.get(0))?;
        Ok((agents, pending, total))
    }

    pub fn cleanup_expired_bridge_messages(&self) -> Result<()> {
        let updated = self.conn.execute(
            "UPDATE bridge_messages SET state = 'expired', claimed_by = NULL,
                    claim_token = NULL, claim_expires_at = NULL
             WHERE acked_at IS NULL AND state IN ('pending', 'claimed')
               AND expires_at IS NOT NULL AND expires_at <= CURRENT_TIMESTAMP",
            [],
        )?;
        // Expired delivery leases are returned to the pending queue so a
        // crashed worker can be retried, unless the message itself expired.
        self.conn.execute(
            "UPDATE bridge_messages SET state = 'pending', claimed_by = NULL,
                    claim_token = NULL, claim_expires_at = NULL
             WHERE state = 'claimed' AND acked_at IS NULL
               AND (claim_expires_at IS NULL OR claim_expires_at <= CURRENT_TIMESTAMP)
               AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)",
            [],
        )?;
        let _ = updated;
        Ok(())
    }

    // Window-based usage tracking
    fn window_start(window: &str) -> String {
        let now = Utc::now();
        match window {
            "per_minute" => format!(
                "{}-{:02}-{:02}T{:02}:{:02}:00Z",
                now.year(),
                now.month(),
                now.day(),
                now.hour(),
                now.minute()
            ),
            "hourly" => format!(
                "{}-{:02}-{:02}T{:02}:00:00Z",
                now.year(),
                now.month(),
                now.day(),
                now.hour()
            ),
            "daily" => format!(
                "{}-{:02}-{:02}T00:00:00Z",
                now.year(),
                now.month(),
                now.day()
            ),
            "monthly" => format!("{}-{:02}-01T00:00:00Z", now.year(), now.month()),
            _ => format!("{}-{:02}-01T00:00:00Z", now.year(), now.month()),
        }
    }

    pub fn record_usage_window(&self, service: &str, tokens: i64, window: &str) -> Result<()> {
        let window_start = Self::window_start(window);
        self.conn.execute(
            "INSERT INTO usage_windows (service, window, window_start, tokens) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(service, window, window_start) DO UPDATE SET tokens = tokens + ?4",
            params![service, window, window_start, tokens],
        )?;
        Ok(())
    }

    pub fn get_usage_window(&self, service: &str, window: &str) -> Result<i64> {
        let window_start = Self::window_start(window);
        // P1.4: Properly handle database errors - only return 0 for missing rows
        match self.conn.query_row(
            "SELECT tokens FROM usage_windows WHERE service = ?1 AND window = ?2 AND window_start = ?3",
            params![service, window, window_start],
            |row| row.get(0),
        ) {
            Ok(usage) => Ok(usage),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(0),
            Err(e) => Err(e.into()),
        }
    }

    // Rate limit observation
    pub fn observe_rate_limit(
        &self,
        service: &str,
        window: &str,
        max_tokens: i64,
        source: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO rate_limits (service, window, max_tokens, source, last_observed)
             VALUES (?1, ?2, ?3, ?4, CURRENT_TIMESTAMP)
             ON CONFLICT(service, window) DO UPDATE SET
                max_tokens = ?3,
                source = ?4,
                last_observed = CURRENT_TIMESTAMP",
            params![service, window, max_tokens, source],
        )?;
        Ok(())
    }

    pub fn get_observed_limit(&self, service: &str, window: &str) -> Result<Option<i64>> {
        let limit: Option<i64> = self
            .conn
            .query_row(
                "SELECT max_tokens FROM rate_limits WHERE service = ?1 AND window = ?2",
                params![service, window],
                |row| row.get(0),
            )
            .ok();
        Ok(limit)
    }

    // Tier management
    pub fn set_tier(&self, service: &str, tier_name: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO tiers (service, tier_name, updated_at)
             VALUES (?1, ?2, CURRENT_TIMESTAMP)
             ON CONFLICT(service) DO UPDATE SET
                tier_name = ?2,
                updated_at = CURRENT_TIMESTAMP",
            params![service, tier_name],
        )?;
        Ok(())
    }

    pub fn get_tier(&self, service: &str) -> Result<Option<String>> {
        let tier: Option<String> = self
            .conn
            .query_row(
                "SELECT tier_name FROM tiers WHERE service = ?1",
                params![service],
                |row| row.get(0),
            )
            .ok();
        Ok(tier)
    }

    // Clean up old window data
    pub fn cleanup_old_windows(&self) -> Result<()> {
        // Delete window data older than 2 days
        self.conn.execute(
            "DELETE FROM usage_windows WHERE window_start < datetime('now', '-2 days')",
            [],
        )?;
        Ok(())
    }

    // High-value feature: Cost tracking
    pub fn get_cost_summary(&self, days: Option<i32>) -> Result<Vec<CostSummary>> {
        let query = if let Some(d) = days {
            format!(
                "SELECT service, SUM(cost_usd) as total_cost, SUM(tokens) as total_tokens, COUNT(*) as request_count
                 FROM history
                 WHERE timestamp > datetime('now', '-{} days')
                 GROUP BY service
                 ORDER BY total_cost DESC",
                d
            )
        } else {
            "SELECT service, SUM(cost_usd) as total_cost, SUM(tokens) as total_tokens, COUNT(*) as request_count
             FROM history
             GROUP BY service
             ORDER BY total_cost DESC".to_string()
        };

        let mut stmt = self.conn.prepare(&query)?;
        let summaries = stmt.query_map([], |row| {
            Ok(CostSummary {
                service: row.get(0)?,
                total_cost: row.get(1)?,
                total_tokens: row.get(2)?,
                request_count: row.get(3)?,
            })
        })?;

        let mut results = Vec::new();
        for summary in summaries {
            results.push(summary?);
        }
        Ok(results)
    }

    pub fn get_total_cost(&self, days: Option<i32>) -> Result<f64> {
        let query = if let Some(d) = days {
            format!(
                "SELECT SUM(cost_usd) FROM history WHERE timestamp > datetime('now', '-{} days')",
                d
            )
        } else {
            "SELECT SUM(cost_usd) FROM history".to_string()
        };

        let total: f64 = self
            .conn
            .query_row(&query, [], |row| row.get(0))
            .unwrap_or(0.0);
        Ok(total)
    }

    // High-value feature: Success/failure tracking
    pub fn get_success_rates(&self) -> Result<Vec<SuccessRate>> {
        let mut stmt = self.conn.prepare(
            "SELECT service,
                    COUNT(*) as total,
                    SUM(CASE WHEN exit_code = 0 THEN 1 ELSE 0 END) as successful,
                    SUM(CASE WHEN exit_code != 0 THEN 1 ELSE 0 END) as failed
             FROM history
             GROUP BY service",
        )?;

        let rates = stmt.query_map([], |row| {
            let total: i64 = row.get(1)?;
            let successful: i64 = row.get(2)?;
            let failed: i64 = row.get(3)?;
            let success_rate = if total > 0 {
                (successful as f64 / total as f64) * 100.0
            } else {
                0.0
            };

            Ok(SuccessRate {
                service: row.get(0)?,
                total_requests: total,
                successful,
                failed,
                success_rate,
            })
        })?;

        let mut results = Vec::new();
        for rate in rates {
            results.push(rate?);
        }
        Ok(results)
    }

    // High-value feature: Hit rate distribution
    pub fn get_hit_rates(&self) -> Result<Vec<HitRate>> {
        let total: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM history", [], |row| row.get(0))
            .unwrap_or(0);

        let mut stmt = self.conn.prepare(
            "SELECT service, COUNT(*) as count
             FROM history
             GROUP BY service
             ORDER BY count DESC",
        )?;

        let rates = stmt.query_map([], |row| {
            let count: i64 = row.get(1)?;
            let percentage = if total > 0 {
                (count as f64 / total as f64) * 100.0
            } else {
                0.0
            };

            Ok(HitRate {
                service: row.get(0)?,
                count,
                percentage,
            })
        })?;

        let mut results = Vec::new();
        for rate in rates {
            results.push(rate?);
        }
        Ok(results)
    }

    pub fn get_task_distribution(&self) -> Result<Vec<TaskDistribution>> {
        let total: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM history", [], |row| row.get(0))
            .unwrap_or(0);

        let mut stmt = self.conn.prepare(
            "SELECT task_type, COUNT(*) as count
             FROM history
             GROUP BY task_type
             ORDER BY count DESC",
        )?;

        let distributions = stmt.query_map([], |row| {
            let count: i64 = row.get(1)?;
            let percentage = if total > 0 {
                (count as f64 / total as f64) * 100.0
            } else {
                0.0
            };

            Ok(TaskDistribution {
                task_type: row.get(0)?,
                count,
                percentage,
            })
        })?;

        let mut results = Vec::new();
        for dist in distributions {
            results.push(dist?);
        }
        Ok(results)
    }

    // High-value feature: Response caching
    // Cache persistence is ready for a future cache-aware routing policy.
    #[allow(dead_code)]
    pub fn get_cached_response(
        &self,
        prompt_hash: &str,
        service: &str,
    ) -> Result<Option<(String, i64)>> {
        let result: Option<(String, i64)> = self
            .conn
            .query_row(
                "SELECT response, tokens FROM cache
             WHERE prompt_hash = ?1 AND service = ?2 AND expires_at > CURRENT_TIMESTAMP",
                params![prompt_hash, service],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        // Increment hit count if found
        if result.is_some() {
            self.conn.execute(
                "UPDATE cache SET hit_count = hit_count + 1 WHERE prompt_hash = ?1 AND service = ?2",
                params![prompt_hash, service],
            )?;
        }

        Ok(result)
    }

    #[allow(dead_code)]
    pub fn cache_response(
        &self,
        prompt_hash: &str,
        service: &str,
        prompt: &str,
        response: &str,
        tokens: i64,
        ttl_hours: i32,
    ) -> Result<()> {
        if prompt_hash.is_empty() || service.is_empty() {
            bail!("cache key and service must not be empty");
        }
        if tokens < 0 {
            bail!("cached token count must not be negative");
        }
        if !(1..=24 * 30).contains(&ttl_hours) {
            bail!("cache TTL must be between 1 and 720 hours");
        }

        self.conn.execute(
            "INSERT INTO cache (prompt_hash, service, prompt, response, tokens, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, datetime('now', '+' || ?6 || ' hours'))
             ON CONFLICT(prompt_hash, service) DO UPDATE SET
                response = ?4,
                tokens = ?5,
                expires_at = datetime('now', '+' || ?6 || ' hours'),
                hit_count = 0",
            params![prompt_hash, service, prompt, response, tokens, ttl_hours],
        )?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn cleanup_expired_cache(&self) -> Result<()> {
        self.conn
            .execute("DELETE FROM cache WHERE expires_at < CURRENT_TIMESTAMP", [])?;
        Ok(())
    }

    pub fn get_cache_stats(&self) -> Result<(i64, i64, i64)> {
        let total: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM cache", [], |row| row.get(0))
            .unwrap_or(0);
        let hits: i64 =
            self.conn
                .query_row("SELECT COALESCE(SUM(hit_count), 0) FROM cache", [], |row| {
                    row.get(0)
                })?;
        let saved_tokens: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(tokens * hit_count), 0) FROM cache",
            [],
            |row| row.get(0),
        )?;
        Ok((total, hits, saved_tokens))
    }

    // Medium-value feature: Budget alerts
    pub fn check_budget_alerts(&self, service: &str) -> Result<Vec<i32>> {
        let usage = self.get_usage(service)?;
        let budget = self.get_budget(service)?;

        if budget == 0 {
            return Ok(vec![]);
        }

        let pct = (usage as f64 / budget as f64 * 100.0) as i32;
        let mut alerts = Vec::new();

        // Check alert thresholds
        let (sent_50, sent_75, sent_90): (bool, bool, bool) = self.conn.query_row(
            "SELECT alert_50_sent, alert_75_sent, alert_90_sent FROM budget_alerts WHERE service = ?1",
            params![service],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        ).unwrap_or((false, false, false));

        if pct >= 90 && !sent_90 {
            alerts.push(90);
        }
        if pct >= 75 && !sent_75 {
            alerts.push(75);
        }
        if pct >= 50 && !sent_50 {
            alerts.push(50);
        }

        Ok(alerts)
    }

    pub fn mark_alert_sent(&self, service: &str, threshold: i32) -> Result<()> {
        // Ensure row exists
        self.conn.execute(
            "INSERT OR IGNORE INTO budget_alerts (service) VALUES (?1)",
            params![service],
        )?;

        // Use explicit SQL statements instead of string formatting to prevent SQL injection
        match threshold {
            50 => self.conn.execute(
                "UPDATE budget_alerts SET alert_50_sent = 1 WHERE service = ?1",
                params![service],
            )?,
            75 => self.conn.execute(
                "UPDATE budget_alerts SET alert_75_sent = 1 WHERE service = ?1",
                params![service],
            )?,
            90 => self.conn.execute(
                "UPDATE budget_alerts SET alert_90_sent = 1 WHERE service = ?1",
                params![service],
            )?,
            _ => return Ok(()),
        };

        Ok(())
    }

    #[allow(dead_code)]
    pub fn reset_budget_alerts(&self, service: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE budget_alerts SET alert_50_sent = 0, alert_75_sent = 0, alert_90_sent = 0, last_reset = CURRENT_TIMESTAMP WHERE service = ?1",
            params![service],
        )?;
        Ok(())
    }

    // Medium-value feature: Service health monitoring
    #[allow(dead_code)]
    pub fn record_health_metric(
        &self,
        service: &str,
        response_time_ms: i64,
        success: bool,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO health_metrics (service, response_time_ms, success) VALUES (?1, ?2, ?3)",
            params![service, response_time_ms, success],
        )?;
        Ok(())
    }

    pub fn get_health_metrics(&self, hours: i32) -> Result<Vec<HealthMetric>> {
        let mut stmt = self.conn.prepare(
            "SELECT service,
                    AVG(response_time_ms) as avg_time,
                    SUM(CASE WHEN success = 1 THEN 1 ELSE 0 END) as successes,
                    SUM(CASE WHEN success = 0 THEN 1 ELSE 0 END) as failures,
                    COUNT(*) as total
             FROM health_metrics
             WHERE timestamp > datetime('now', '-' || ?1 || ' hours')
             GROUP BY service",
        )?;

        let metrics = stmt.query_map(params![hours], |row| {
            let successes: i64 = row.get(2)?;
            let failures: i64 = row.get(3)?;
            let total: i64 = row.get(4)?;
            let success_rate = if total > 0 {
                (successes as f64 / total as f64) * 100.0
            } else {
                0.0
            };

            Ok(HealthMetric {
                service: row.get(0)?,
                avg_response_time_ms: row.get(1)?,
                success_rate,
                recent_failures: failures,
            })
        })?;

        let mut results = Vec::new();
        for metric in metrics {
            results.push(metric?);
        }
        Ok(results)
    }

    #[allow(dead_code)]
    pub fn cleanup_old_health_metrics(&self) -> Result<()> {
        self.conn.execute(
            "DELETE FROM health_metrics WHERE timestamp < datetime('now', '-7 days')",
            [],
        )?;
        Ok(())
    }

    // Medium-value feature: Configuration profiles
    pub fn create_profile(&self, name: &str) -> Result<()> {
        self.conn
            .execute("INSERT INTO profiles (name) VALUES (?1)", params![name])?;
        Ok(())
    }

    pub fn list_profiles(&self) -> Result<Vec<Profile>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, active FROM profiles ORDER BY name")?;
        let profiles = stmt.query_map([], |row| {
            Ok(Profile {
                name: row.get(0)?,
                active: row.get(1)?,
            })
        })?;

        let mut results = Vec::new();
        for profile in profiles {
            results.push(profile?);
        }
        Ok(results)
    }

    pub fn switch_profile(&self, name: &str) -> Result<()> {
        // Deactivate all profiles
        self.conn.execute("UPDATE profiles SET active = 0", [])?;

        // Activate the specified profile
        self.conn.execute(
            "UPDATE profiles SET active = 1 WHERE name = ?1",
            params![name],
        )?;

        // Load budgets from profile
        let mut stmt = self
            .conn
            .prepare("SELECT service, tokens FROM profile_budgets WHERE profile = ?1")?;

        let budgets = stmt.query_map(params![name], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;

        for budget in budgets {
            let (service, tokens) = budget?;
            self.set_budget(&service, tokens)?;
        }

        // Load tiers from profile
        let mut stmt = self
            .conn
            .prepare("SELECT service, tier_name FROM profile_tiers WHERE profile = ?1")?;

        let tiers = stmt.query_map(params![name], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        for tier in tiers {
            let (service, tier_name) = tier?;
            self.set_tier(&service, &tier_name)?;
        }

        Ok(())
    }

    pub fn save_current_to_profile(&self, name: &str) -> Result<()> {
        // Save current budgets to profile
        for service in crate::constants::SERVICES {
            let budget = self.get_budget(service)?;
            self.conn.execute(
                "INSERT OR REPLACE INTO profile_budgets (profile, service, tokens) VALUES (?1, ?2, ?3)",
                params![name, service, budget],
            )?;

            if let Some(tier) = self.get_tier(service)? {
                self.conn.execute(
                    "INSERT OR REPLACE INTO profile_tiers (profile, service, tier_name) VALUES (?1, ?2, ?3)",
                    params![name, service, tier],
                )?;
            }
        }
        Ok(())
    }

    // Medium-value feature: Prompt templates
    pub fn save_template(
        &self,
        name: &str,
        prompt: &str,
        description: Option<&str>,
        variables: &[String],
    ) -> Result<()> {
        let vars_json = serde_json::to_string(variables)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO templates (name, prompt, description, variables) VALUES (?1, ?2, ?3, ?4)",
            params![name, prompt, description, vars_json],
        )?;
        Ok(())
    }

    pub fn get_template(&self, name: &str) -> Result<Option<Template>> {
        let result: Option<Template> = self
            .conn
            .query_row(
                "SELECT name, prompt, description, variables FROM templates WHERE name = ?1",
                params![name],
                |row| {
                    let vars_json: String = row.get(3)?;
                    let variables: Vec<String> =
                        serde_json::from_str(&vars_json).unwrap_or_default();
                    Ok(Template {
                        name: row.get(0)?,
                        prompt: row.get(1)?,
                        description: row.get(2)?,
                        variables,
                    })
                },
            )
            .optional()?;

        // Update last_used
        if result.is_some() {
            self.conn.execute(
                "UPDATE templates SET last_used = CURRENT_TIMESTAMP WHERE name = ?1",
                params![name],
            )?;
        }

        Ok(result)
    }

    pub fn list_templates(&self) -> Result<Vec<Template>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, prompt, description, variables FROM templates ORDER BY last_used DESC",
        )?;

        let templates = stmt.query_map([], |row| {
            let vars_json: String = row.get(3)?;
            let variables: Vec<String> = serde_json::from_str(&vars_json).unwrap_or_default();
            Ok(Template {
                name: row.get(0)?,
                prompt: row.get(1)?,
                description: row.get(2)?,
                variables,
            })
        })?;

        let mut results = Vec::new();
        for template in templates {
            results.push(template?);
        }
        Ok(results)
    }

    pub fn delete_template(&self, name: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM templates WHERE name = ?1", params![name])?;
        Ok(())
    }

    // Medium-value feature: Service aliases
    pub fn set_alias(&self, alias: &str, service: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO aliases (alias, service) VALUES (?1, ?2)",
            params![alias, service],
        )?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_alias(&self, alias: &str) -> Result<Option<String>> {
        let service: Option<String> = self
            .conn
            .query_row(
                "SELECT service FROM aliases WHERE alias = ?1",
                params![alias],
                |row| row.get(0),
            )
            .optional()?;
        Ok(service)
    }

    pub fn list_aliases(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT alias, service FROM aliases ORDER BY alias")?;
        let aliases = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;

        let mut results = Vec::new();
        for alias in aliases {
            results.push(alias?);
        }
        Ok(results)
    }

    pub fn delete_alias(&self, alias: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM aliases WHERE alias = ?1", params![alias])?;
        Ok(())
    }
}

fn add_bridge_column_if_missing(conn: &Connection, sql: &str) -> Result<()> {
    match conn.execute(sql, []) {
        Ok(_) => Ok(()),
        Err(rusqlite::Error::SqliteFailure(_, Some(message)))
            if message
                .to_ascii_lowercase()
                .contains("duplicate column name") =>
        {
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_database() -> Database {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE bridge_agents (
                agent_id TEXT PRIMARY KEY,
                display_name TEXT NOT NULL,
                provider TEXT NOT NULL,
                model_id TEXT NOT NULL,
                session_key TEXT,
                capabilities TEXT NOT NULL DEFAULT '[]',
                status TEXT NOT NULL DEFAULT 'online',
                last_seen_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE bridge_messages (
                message_id TEXT PRIMARY KEY,
                from_agent TEXT NOT NULL,
                to_agent TEXT NOT NULL,
                task_id TEXT,
                message_type TEXT NOT NULL,
                payload TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                expires_at DATETIME,
                delivered_at DATETIME,
                acked_at DATETIME,
                attempts INTEGER NOT NULL DEFAULT 0,
                state TEXT NOT NULL DEFAULT 'pending',
                claimed_by TEXT,
                claim_token TEXT,
                claim_expires_at DATETIME
            );",
        )
        .unwrap();
        Database { conn }
    }

    #[test]
    fn bridge_claim_and_ack_are_recipient_scoped() {
        let db = test_database();
        db.register_agent("sender", "sender", "codex", "default", None, &[])
            .unwrap();
        db.register_agent("receiver", "receiver", "claude", "default", None, &[])
            .unwrap();
        db.send_bridge_message(
            "msg_test", "sender", "receiver", None, "request", "payload", None,
        )
        .unwrap();

        let claimed = db.get_bridge_inbox("receiver", false).unwrap();
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].state, "claimed");
        let claim_token = claimed[0].claim_token.clone().unwrap();
        assert!(!db
            .ack_bridge_message("sender", "msg_test", &claim_token)
            .unwrap());
        assert!(db
            .ack_bridge_message("receiver", "msg_test", &claim_token)
            .unwrap());
        assert!(db.get_bridge_inbox("receiver", false).unwrap().is_empty());
    }

    #[test]
    fn expired_messages_are_retained_as_terminal_records() {
        let db = test_database();
        db.register_agent("sender", "sender", "codex", "default", None, &[])
            .unwrap();
        db.register_agent("receiver", "receiver", "claude", "default", None, &[])
            .unwrap();
        db.send_bridge_message(
            "msg_expired",
            "sender",
            "receiver",
            None,
            "request",
            "payload",
            Some("2000-01-01 00:00:00"),
        )
        .unwrap();
        db.cleanup_expired_bridge_messages().unwrap();
        let history = db.get_bridge_inbox("receiver", true).unwrap();
        assert_eq!(history[0].state, "expired");
    }

    #[test]
    fn expired_delivery_lease_returns_message_to_pending_queue() {
        let db = test_database();
        db.register_agent("sender", "sender", "codex", "default", None, &[])
            .unwrap();
        db.register_agent("receiver", "receiver", "claude", "default", None, &[])
            .unwrap();
        db.send_bridge_message(
            "msg_retry",
            "sender",
            "receiver",
            None,
            "request",
            "payload",
            None,
        )
        .unwrap();

        let first = db.get_bridge_inbox("receiver", false).unwrap();
        let first_token = first[0].claim_token.clone().unwrap();
        db.conn
            .execute(
                "UPDATE bridge_messages SET claim_expires_at = '2000-01-01 00:00:00' WHERE message_id = 'msg_retry'",
                [],
            )
            .unwrap();
        db.cleanup_expired_bridge_messages().unwrap();

        let second = db.get_bridge_inbox("receiver", false).unwrap();
        assert_eq!(second[0].attempts, 2);
        assert_ne!(second[0].claim_token.as_deref(), Some(first_token.as_str()));
    }

    #[test]
    fn acknowledgements_require_the_current_claim_token() {
        let db = test_database();
        db.register_agent("sender", "sender", "codex", "default", None, &[])
            .unwrap();
        db.register_agent("receiver", "receiver", "claude", "default", None, &[])
            .unwrap();
        db.send_bridge_message(
            "msg_token",
            "sender",
            "receiver",
            None,
            "request",
            "payload",
            None,
        )
        .unwrap();
        let claimed = db.get_bridge_inbox("receiver", false).unwrap();
        let token = claimed[0].claim_token.as_deref().unwrap();
        assert!(!db
            .ack_bridge_message("receiver", "msg_token", "wrong")
            .unwrap());
        assert!(db
            .ack_bridge_message("receiver", "msg_token", token)
            .unwrap());
    }

    #[test]
    fn expired_claim_tokens_cannot_ack_before_cleanup() {
        let db = test_database();
        db.register_agent("sender", "sender", "codex", "default", None, &[])
            .unwrap();
        db.register_agent("receiver", "receiver", "claude", "default", None, &[])
            .unwrap();
        db.send_bridge_message(
            "msg_expired_claim",
            "sender",
            "receiver",
            None,
            "request",
            "payload",
            None,
        )
        .unwrap();
        let claimed = db.get_bridge_inbox("receiver", false).unwrap();
        let token = claimed[0].claim_token.clone().unwrap();
        db.conn
            .execute(
                "UPDATE bridge_messages SET claim_expires_at = '2000-01-01 00:00:00' WHERE message_id = 'msg_expired_claim'",
                [],
            )
            .unwrap();
        assert!(!db
            .ack_bridge_message("receiver", "msg_expired_claim", &token)
            .unwrap());
        db.cleanup_expired_bridge_messages().unwrap();
        let reclaimed = db.get_bridge_inbox("receiver", false).unwrap();
        assert!(db
            .ack_bridge_message(
                "receiver",
                "msg_expired_claim",
                reclaimed[0].claim_token.as_deref().unwrap(),
            )
            .unwrap());
    }

    #[test]
    fn unregistering_agent_marks_undeliverable_messages() {
        let db = test_database();
        db.register_agent("sender", "sender", "codex", "default", None, &[])
            .unwrap();
        db.register_agent("receiver", "receiver", "claude", "default", None, &[])
            .unwrap();
        db.send_bridge_message(
            "msg_orphan",
            "sender",
            "receiver",
            None,
            "request",
            "payload",
            None,
        )
        .unwrap();
        assert!(db.unregister_agent("receiver").unwrap());
        let messages = db.get_bridge_inbox("receiver", true).unwrap();
        assert_eq!(messages[0].state, "undeliverable");
    }

    #[test]
    fn bridge_migration_only_ignores_duplicate_columns() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE bridge_messages (message_id TEXT)", [])
            .unwrap();
        assert!(add_bridge_column_if_missing(
            &conn,
            "ALTER TABLE bridge_messages ADD COLUMN state TEXT"
        )
        .is_ok());
        assert!(add_bridge_column_if_missing(
            &conn,
            "ALTER TABLE bridge_messages ADD COLUMN state TEXT"
        )
        .is_ok());
        assert!(add_bridge_column_if_missing(
            &conn,
            "ALTER TABLE missing_table ADD COLUMN state TEXT"
        )
        .is_err());
    }
}
