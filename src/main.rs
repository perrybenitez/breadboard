use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;

mod bridge;
mod compress;
mod config;
mod constants;
#[cfg(feature = "dashboard")]
mod dashboard;
mod db;
mod live;
mod models;
mod router;
mod sessions;
#[cfg(feature = "sync")]
mod sync;
mod tiers;

use config::Config;
use db::Database;
use router::{Router, TaskType};

#[derive(Parser)]
#[command(name = "breadboard")]
#[command(version)]
#[command(about = "Breadboard - Routing engine for Claude/Codex/Cursor with budget management")]
#[command(
    long_about = "Breadboard routes your prompts to Claude, Codex, or Cursor based on task type and budget constraints.\n\nGetting Started:\n  breadboard setup           # Interactive setup wizard\n  breadboard doctor          # Validate configuration\n  breadboard --dry-run \"...\" # Test routing decisions\n\nFull documentation: https://github.com/..."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Dry-run mode: show routing decision without executing
    #[arg(long, global = true)]
    dry_run: bool,

    /// Override the configured model for the selected service
    #[arg(long, global = true)]
    model: Option<String>,

    /// Resume a named Breadboard session
    #[arg(long, global = true, value_name = "NAME")]
    session: Option<String>,

    /// Start a new session instead of resuming an existing matching session
    #[arg(long, global = true, requires = "session")]
    new_session: bool,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Direct prompt (routes automatically)
    #[arg(trailing_var_arg = true)]
    prompt: Vec<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// View current usage and budgets
    Status,

    /// Set monthly budget for a service (in tokens)
    SetBudget { service: String, tokens: i64 },

    /// Reset usage counters
    Reset {
        #[arg(long)]
        service: Option<String>,
    },

    /// Configure API settings
    Config,

    /// Show routing history
    History {
        #[arg(long, default_value = "10")]
        limit: usize,
    },

    /// Set subscription tier for a service
    SetTier { service: String, tier: String },

    /// Show limits (observed from APIs and tier-based)
    Limits,

    /// Interactive dashboard with live metrics
    #[cfg(feature = "dashboard")]
    Dashboard,

    /// Sync usage from service APIs
    #[cfg(feature = "sync")]
    Sync,

    /// Show statistics (hit rates, success rates, task distribution)
    Stats,

    /// Show cost analysis and savings
    Costs {
        #[arg(long)]
        days: Option<i32>,
    },

    /// Export usage data to CSV
    Export {
        #[arg(long, default_value = "usage_export.csv")]
        output: String,
    },

    /// Service health monitoring
    Health {
        #[arg(long, default_value = "24")]
        hours: i32,
    },

    /// Configuration profile management
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },

    /// Prompt template management
    Template {
        #[command(subcommand)]
        action: TemplateAction,
    },

    /// Service alias management
    Alias {
        #[command(subcommand)]
        action: AliasAction,
    },

    /// Interactive setup wizard (recommended for first-time users)
    Setup,

    /// Validate configuration and check system health
    Doctor,

    /// Manage persistent Breadboard sessions
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },

    /// Discover model IDs exposed by Claude, Codex, or Cursor
    Models {
        /// Provider to inspect; omit to inspect all providers
        service: Option<String>,
    },

    /// Coordinate messages between registered agent sessions
    Bridge {
        #[command(subcommand)]
        action: BridgeAction,
    },
}

#[derive(Subcommand)]
enum ProfileAction {
    /// Create a new profile
    Create { name: String },
    /// List all profiles
    List,
    /// Switch to a profile
    Switch { name: String },
    /// Save current config to profile
    Save { name: String },
}

#[derive(Subcommand)]
enum TemplateAction {
    /// Save a prompt template
    Save {
        name: String,
        prompt: String,
        #[arg(long)]
        description: Option<String>,
    },
    /// Use a template
    Use {
        name: String,
        /// Variable substitutions in format var=value
        #[arg(trailing_var_arg = true)]
        vars: Vec<String>,
    },
    /// List all templates
    List,
    /// Delete a template
    Delete { name: String },
}

#[derive(Subcommand)]
enum AliasAction {
    /// Set a service alias
    Set { alias: String, service: String },
    /// List all aliases
    List,
    /// Delete an alias
    Delete { alias: String },
}

#[derive(Subcommand)]
enum SessionAction {
    /// List session keys and their provider mappings
    List,
    /// Delete a session by its Breadboard session key
    Delete { key: String },
}

#[derive(Subcommand)]
enum BridgeAction {
    /// Show bridge and message queue status
    Status,
    /// Manage registered agents
    Agent {
        #[command(subcommand)]
        action: AgentAction,
    },
    /// Send an explicit message to a registered agent
    Send {
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        payload: String,
        #[arg(long, default_value = "request")]
        message_type: String,
        #[arg(long)]
        task: Option<String>,
        #[arg(long)]
        ttl: Option<u64>,
    },
    /// Read pending messages for an agent
    Inbox {
        agent: String,
        #[arg(long)]
        all: bool,
    },
    /// Acknowledge a delivered message owned by an agent
    Ack {
        agent: String,
        message_id: String,
        #[arg(long)]
        claim_token: String,
    },
    /// Run the stdio tool server for agent integrations
    Mcp,
    /// Run a persistent provider session with turn-boundary bridge delivery
    Run {
        /// Provider to keep alive (codex or claude)
        #[arg(long)]
        provider: String,
        /// Optional provider model ID
        #[arg(long)]
        model: Option<String>,
        /// Named Breadboard session to open or resume
        #[arg(long)]
        session: String,
    },
}

#[derive(Subcommand)]
enum AgentAction {
    /// Register or update an agent
    Register {
        agent_id: String,
        #[arg(long)]
        provider: String,
        #[arg(long, default_value = "default")]
        model: String,
        #[arg(long)]
        session: Option<String>,
        #[arg(long = "capability", short = 'c')]
        capabilities: Vec<String>,
    },
    /// List registered agents
    List,
    /// Remove an agent registration
    Unregister { agent_id: String },
    /// Refresh an agent's online lease
    Heartbeat { agent_id: String },
}

fn main() -> Result<()> {
    // Load .env file if it exists (for API keys)
    #[cfg(feature = "sync")]
    let _ = dotenvy::dotenv();

    let cli = Cli::parse();

    // P2.2: Initialize logger based on verbose flag
    if cli.verbose {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Debug)
            .init();
        log::debug!("Verbose logging enabled");
    } else {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Warn)
            .init();
    }

    log::debug!("Loading configuration");
    let config = Config::load()?;

    log::debug!("Initializing database");
    let db = Database::new()?;

    match cli.command {
        Some(Commands::Status) => {
            show_status(&db, &config)?;
        }
        Some(Commands::SetBudget { service, tokens }) => {
            // P2.5: Validate service name
            if !constants::is_valid_service(&service) {
                println!("{} Unknown service: {}", "[ERROR]".red(), service);
                if let Some(suggestion) = constants::suggest_service(&service) {
                    println!("   Did you mean: {}?", suggestion.yellow());
                }
                println!(
                    "   Valid services: {}",
                    constants::SERVICES.join(", ").cyan()
                );
                std::process::exit(1);
            }

            db.set_budget(&service, tokens)?;
            println!(
                "{} Budget set: {} = {} tokens/month",
                "[OK]".green(),
                service.cyan(),
                tokens.to_string().yellow()
            );
        }
        Some(Commands::Reset { service }) => {
            if let Some(svc) = service {
                // P2.5: Validate service name
                if !constants::is_valid_service(&svc) {
                    println!("{} Unknown service: {}", "[ERROR]".red(), svc);
                    if let Some(suggestion) = constants::suggest_service(&svc) {
                        println!("   Did you mean: {}?", suggestion.yellow());
                    }
                    println!(
                        "   Valid services: {}",
                        constants::SERVICES.join(", ").cyan()
                    );
                    std::process::exit(1);
                }

                db.reset_usage(&svc)?;
                println!("{} Reset usage for {}", "[OK]".green(), svc.cyan());
            } else {
                db.reset_all_usage()?;
                println!("{} Reset all usage counters", "[OK]".green());
            }
        }
        Some(Commands::Config) => {
            config::interactive_config()?;
        }
        Some(Commands::History { limit }) => {
            show_history(&db, limit)?;
        }
        Some(Commands::SetTier { service, tier }) => {
            use tiers::Tier;

            // P2.5: Validate service name
            if !constants::is_valid_service(&service) {
                println!("{} Unknown service: {}", "[ERROR]".red(), service);
                if let Some(suggestion) = constants::suggest_service(&service) {
                    println!("   Did you mean: {}?", suggestion.yellow());
                }
                println!(
                    "   Valid services: {}",
                    constants::SERVICES.join(", ").cyan()
                );
                std::process::exit(1);
            }

            if Tier::from_string(&tier).is_some() {
                db.set_tier(&service, &tier)?;
                println!(
                    "{} Tier set: {} = {}",
                    "[OK]".green(),
                    service.cyan(),
                    tier.yellow()
                );
            } else {
                println!("{} Unknown tier: {}", "[ERROR]".red(), tier);
                println!(
                    "Valid tiers: claude-pro, claude-team, cursor-pro, cursor-business, codex-payg"
                );
            }
        }
        Some(Commands::Limits) => {
            show_limits(&db)?;
        }
        #[cfg(feature = "dashboard")]
        Some(Commands::Dashboard) => {
            dashboard::run_dashboard(&db)?;
        }
        #[cfg(feature = "sync")]
        Some(Commands::Sync) => {
            println!("{} Syncing usage from APIs...\n", "[SYNC]".cyan());
            let results = sync::sync_all(&db)?;

            for result in results {
                if result.synced {
                    println!("{} {} - In sync", "[OK]".green(), result.service.cyan());
                    println!(
                        "   API usage: {} tokens",
                        result.api_usage.to_string().yellow()
                    );
                    println!(
                        "   Local usage: {} tokens",
                        result.local_usage.to_string().dimmed()
                    );
                } else {
                    let status = if result.error.is_some() {
                        "[WARN]".yellow()
                    } else {
                        "[ERROR]".red()
                    };
                    println!("{} {}", status, result.service.cyan());

                    if result.api_usage > 0 || result.local_usage > 0 {
                        let diff = result.api_usage - result.local_usage;
                        println!(
                            "   API usage: {} tokens",
                            result.api_usage.to_string().yellow()
                        );
                        println!(
                            "   Local usage: {} tokens",
                            result.local_usage.to_string().yellow()
                        );

                        if diff != 0 {
                            let diff_str = if diff > 0 {
                                format!("+{}", diff).red()
                            } else {
                                format!("{}", diff).green()
                            };
                            println!("   Difference: {} tokens", diff_str);
                        }
                    }

                    if let Some(err) = result.error {
                        println!("   {}", err.dimmed());
                    }
                }
                println!();
            }
        }
        Some(Commands::Stats) => {
            show_stats(&db)?;
        }
        Some(Commands::Costs { days }) => {
            show_costs(&db, days)?;
        }
        Some(Commands::Export { output }) => {
            let safe_path = validate_export_path(&output)?;
            export_usage(&db, safe_path.to_str().unwrap())?;
            println!(
                "{} Exported usage data to {}",
                "[OK]".green(),
                output.cyan()
            );
        }
        Some(Commands::Health { hours }) => {
            show_health(&db, hours)?;
        }
        Some(Commands::Profile { action }) => {
            handle_profile_action(&db, &action)?;
        }
        Some(Commands::Template { action }) => {
            handle_template_action(
                &db,
                &config,
                &action,
                cli.dry_run,
                cli.model.as_deref(),
                cli.session.as_deref(),
                cli.new_session,
            )?;
        }
        Some(Commands::Alias { action }) => {
            handle_alias_action(&db, &action)?;
        }
        Some(Commands::Setup) => {
            run_setup_wizard(&config, &db)?;
        }
        Some(Commands::Doctor) => {
            run_doctor(&config, &db)?;
        }
        Some(Commands::Session { action }) => {
            handle_session_action(&db, &action)?;
        }
        Some(Commands::Models { service }) => {
            models::run(&config, service.as_deref())?;
        }
        Some(Commands::Bridge { action }) => {
            handle_bridge_action(&db, &config, &action)?;
        }
        None => {
            if cli.prompt.is_empty() {
                println!("{}", "Breadboard - Budget Management for AI CLIs".bold());
                println!();
                println!("{}", "Getting Started:".bold());
                println!("  breadboard setup           # Interactive setup wizard");
                println!("  breadboard doctor          # Validate configuration");
                println!("  breadboard \"your prompt\"   # Route and execute");
                println!();
                println!("{}", "Quick Commands:".bold());
                println!("  breadboard status          # Check usage");
                println!("  breadboard --dry-run \"...\" # Test routing");
                println!("  breadboard stats           # Analytics");
                println!();
                println!("{}", "Full help:".dimmed());
                println!("  breadboard --help");
                std::process::exit(1);
            }

            let prompt = cli.prompt.join(" ");
            route_and_execute(
                &prompt,
                &config,
                &db,
                cli.dry_run,
                cli.model.as_deref(),
                cli.session.as_deref(),
                cli.new_session,
            )?;
        }
    }

    Ok(())
}

fn route_and_execute(
    prompt: &str,
    config: &Config,
    db: &Database,
    dry_run: bool,
    model_override: Option<&str>,
    session_name: Option<&str>,
    new_session: bool,
) -> Result<()> {
    log::debug!(
        "Starting route_and_execute with prompt length: {}",
        prompt.len()
    );
    let router = Router::new(config, db);

    // Classify task type
    let task_type = TaskType::classify(prompt);
    log::info!("Task classified as: {:?}", task_type);
    println!("{} Task: {}", ">>".cyan(), task_type.description().dimmed());

    // Get best available service within budget
    log::debug!("Attempting to route task");
    let service = router.route(task_type, model_override)?;
    log::info!("Routed to service: {}", service.name());
    println!("{} Routing to: {}", ">>".cyan(), service.name().green());
    if let Some(model) = service.model() {
        println!("{} Model: {}", ">>".cyan(), model.yellow());
    }

    let session = session_name
        .map(|name| {
            sessions::open(
                db,
                service.name(),
                service.model(),
                name,
                new_session,
                !dry_run,
            )
        })
        .transpose()?;
    if let Some(session) = &session {
        sessions::print_started(session);
    }

    // P2.4: Dry-run mode - show decision without executing
    if dry_run {
        println!("\n{} Dry-run mode - not executing", "[INFO]".blue());

        // Show current usage
        let usage = db.get_usage(service.name())?;
        let budget = db.get_budget(service.name())?;
        let pct = if budget > 0 {
            (usage as f64 / budget as f64 * 100.0) as i32
        } else {
            0
        };

        println!("\n{}", "Current Usage:".bold());
        println!("  Service: {}", service.name().cyan());
        println!(
            "  Monthly: {}/{} tokens ({}%)",
            usage.to_string().yellow(),
            budget.to_string().dimmed(),
            pct.to_string().yellow()
        );

        // Show tier limits if configured
        use tiers::Tier;
        if let Some(tier_name) = db.get_tier(service.name())? {
            if let Some(tier) = Tier::from_string(&tier_name) {
                println!("\n{}", "Rate Limits:".bold());
                for limit in tier.get_limits() {
                    let window_usage =
                        db.get_usage_window(service.name(), limit.window.as_str())?;
                    let max = db
                        .get_observed_limit(service.name(), limit.window.as_str())?
                        .unwrap_or(limit.max_tokens);
                    let pct = if max > 0 {
                        (window_usage as f64 / max as f64 * 100.0) as i32
                    } else {
                        0
                    };
                    println!(
                        "  {}: {}/{} ({}%)",
                        limit.window.as_str(),
                        window_usage.to_string().yellow(),
                        max.to_string().dimmed(),
                        pct.to_string().yellow()
                    );
                }
            }
        }

        println!(
            "\n{} Estimated tokens: ~{} (based on prompt length)",
            "[INFO]".dimmed(),
            (prompt.len() / 4).to_string().cyan()
        );

        return Ok(());
    }

    let session_agent_id = if let Some(session) = &session {
        let agent_id = bridge::register_session_agent(db, session, task_type.config_key())?;
        println!("{} Agent: {}", ">>".cyan(), agent_id.cyan());
        Some(agent_id)
    } else {
        None
    };

    let (execution_prompt, claimed_bridge_messages) = if let Some(agent_id) = &session_agent_id {
        let (prompt, messages) = bridge::inject_pending_messages(db, agent_id, prompt)?;
        if !messages.is_empty() {
            println!(
                "{} Injected {} bridge message(s) into the session prompt",
                "<<".cyan(),
                messages.len()
            );
        }
        (prompt, messages)
    } else {
        (prompt.to_string(), Vec::new())
    };

    // Execute the command
    let execution = service.execute(&execution_prompt, session.as_ref());
    let (tokens_used, output, rate_limit_info, exit_code) = match execution {
        Ok(result) => result,
        Err(error) => {
            if let Some(agent_id) = &session_agent_id {
                let _ = db.mark_agent_offline(agent_id);
            }
            return Err(error);
        }
    };
    if let Some(agent_id) = &session_agent_id {
        // The provider command has exited, so this invocation is no longer a
        // live receiver. Keep queued messages durable for the next resume.
        db.mark_agent_offline(agent_id)?;
    }

    // Store observed rate limits if available
    if let Some(info) = rate_limit_info {
        for (window, max_tokens) in info.limits {
            db.observe_rate_limit(service.name(), &window, max_tokens, "cli-output")?;
        }
    }

    // Calculate cost based on tier
    let cost_usd = if let Some(tier_name) = db.get_tier(service.name())? {
        if let Some(tier) = tiers::Tier::from_string(&tier_name) {
            tier.calculate_cost(tokens_used)
        } else {
            0.0
        }
    } else {
        0.0
    };

    // Track usage with cost and success info
    let task_type_str = format!("{:?}", task_type).to_lowercase();

    // Compress prompt and output if enabled
    let stored_prompt = if config.compress_history {
        compress::compress_prompt(prompt)
    } else {
        prompt.to_string()
    };

    db.record_usage(
        service.name(),
        tokens_used,
        &stored_prompt,
        exit_code,
        cost_usd,
        &task_type_str,
    )?;
    if let Some(agent_id) = &session_agent_id {
        // Record usage before acknowledging bridge work so a local database
        // failure cannot silently lose accounting for a completed provider
        // turn. Delivery remains at-least-once if acknowledgment fails.
        bridge::acknowledge_messages(db, agent_id, &claimed_bridge_messages)?;
    }
    if let Some(session) = &session {
        db.touch_session(&session.key)?;
    }

    // Show output
    println!("\n{}", output);

    // Show token usage
    let usage = db.get_usage(service.name())?;
    let budget = db.get_budget(service.name())?;
    // P0.2: Guard against division by zero
    let pct = if budget > 0 {
        (usage as f64 / budget as f64 * 100.0) as i32
    } else {
        0
    };

    println!(
        "\n{} {} tokens used ({}/{} - {}%)",
        "[OK]".green(),
        tokens_used.to_string().yellow(),
        usage.to_string().cyan(),
        budget.to_string().cyan(),
        pct.to_string().yellow()
    );

    if pct > 90 {
        println!(
            "{} Budget warning: {}% of monthly limit",
            "[WARN]".yellow(),
            pct
        );
    }

    // Check for budget alerts
    let alerts = db.check_budget_alerts(service.name())?;
    for threshold in alerts {
        println!(
            "\n{} Budget Alert: {}% threshold reached for {}",
            "[ALERT]".yellow(),
            threshold,
            service.name().cyan()
        );
        println!("   Consider switching to a different service or increasing budget");

        // Mark alert as sent
        db.mark_alert_sent(service.name(), threshold)?;
    }

    if let Some(session) = &session {
        sessions::print_finished(session);
    }

    Ok(())
}

fn show_status(db: &Database, _config: &Config) -> Result<()> {
    println!("\n{}", "Token Usage Status".bold().underline());
    println!();

    for service in constants::SERVICES {
        let usage = db.get_usage(service)?;
        let budget = db.get_budget(service)?;
        let pct = if budget > 0 {
            (usage as f64 / budget as f64 * 100.0) as i32
        } else {
            0
        };

        let bar = progress_bar(pct, 30);
        let status = if pct > 90 {
            "[WARN]".yellow()
        } else {
            "[OK]".green()
        };

        println!("{} {} {}", status, service.cyan().bold(), bar);
        println!(
            "   {}/{} tokens ({}%)",
            usage.to_string().yellow(),
            budget.to_string().dimmed(),
            pct.to_string().yellow()
        );
        println!();
    }

    Ok(())
}

fn show_history(db: &Database, limit: usize) -> Result<()> {
    let history = db.get_history(limit)?;

    println!("\n{}", "Recent Routing History".bold().underline());
    println!();

    for entry in history {
        println!(
            "{} {} >> {} ({} tokens)",
            entry.timestamp.dimmed(),
            entry.prompt_preview.cyan(),
            entry.service.green(),
            entry.tokens.to_string().yellow()
        );
    }

    Ok(())
}

fn progress_bar(pct: i32, width: usize) -> String {
    let filled = (pct as f64 / 100.0 * width as f64) as usize;
    let empty = width.saturating_sub(filled);

    let color = if pct > 90 {
        "red"
    } else if pct > 70 {
        "yellow"
    } else {
        "green"
    };

    let bar = match color {
        "red" => format!("{}{}", "█".repeat(filled).red(), "░".repeat(empty).dimmed()),
        "yellow" => format!(
            "{}{}",
            "█".repeat(filled).yellow(),
            "░".repeat(empty).dimmed()
        ),
        _ => format!(
            "{}{}",
            "█".repeat(filled).green(),
            "░".repeat(empty).dimmed()
        ),
    };

    format!("[{}]", bar)
}

fn show_limits(db: &Database) -> Result<()> {
    use tiers::{RefreshWindow, Tier};

    println!("\n{}", "Service Limits".bold().underline());
    println!();

    for service in constants::SERVICES {
        println!("{}", service.cyan().bold());

        // Get configured tier
        let tier_name = db.get_tier(service)?;
        if let Some(tier_str) = &tier_name {
            if let Some(tier) = Tier::from_string(tier_str) {
                println!("  Tier: {}", tier_str.yellow());

                let limits = tier.get_limits();
                for limit in limits {
                    let window_str = match limit.window {
                        RefreshWindow::PerMinute => "per minute",
                        RefreshWindow::Hourly => "hourly",
                        RefreshWindow::Daily => "daily",
                        RefreshWindow::Monthly => "monthly",
                    };

                    let usage = db.get_usage_window(service, limit.window.as_str())?;
                    let pct = if limit.max_tokens > 0 {
                        (usage as f64 / limit.max_tokens as f64 * 100.0) as i32
                    } else {
                        0
                    };

                    // Check if we have observed limits
                    let observed = db.get_observed_limit(service, limit.window.as_str())?;
                    let limit_display = if let Some(obs) = observed {
                        if obs != limit.max_tokens {
                            format!(
                                "{} {} (observed: {})",
                                limit.max_tokens.to_string().yellow(),
                                window_str.dimmed(),
                                obs.to_string().green()
                            )
                        } else {
                            format!(
                                "{} {}",
                                limit.max_tokens.to_string().green(),
                                window_str.dimmed()
                            )
                        }
                    } else {
                        format!(
                            "{} {} {}",
                            limit.max_tokens.to_string().yellow(),
                            window_str.dimmed(),
                            "(tier default)".dimmed()
                        )
                    };

                    println!(
                        "  {} - {}/{} ({}%)",
                        limit_display,
                        usage.to_string().cyan(),
                        limit.max_tokens.to_string().dimmed(),
                        pct.to_string().yellow()
                    );
                }
            }
        } else {
            println!(
                "  {}",
                "No tier configured - use 'breadboard set-tier'".dimmed()
            );
        }
        println!();
    }

    Ok(())
}

fn show_stats(db: &Database) -> Result<()> {
    println!("\n{}", "Breadboard Statistics".bold().underline());
    println!();

    // Hit rates
    println!("{}", "Service Distribution".bold());
    let hit_rates = db.get_hit_rates()?;
    if hit_rates.is_empty() {
        println!("  No usage data yet");
    } else {
        for rate in hit_rates {
            let bar_width = (rate.percentage / 2.0) as usize;
            let bar = "█".repeat(bar_width);
            println!(
                "  {} {} ({} requests, {:.1}%)",
                rate.service.cyan().bold(),
                bar.green(),
                rate.count.to_string().yellow(),
                rate.percentage
            );
        }
    }
    println!();

    // Task distribution
    println!("{}", "Task Type Distribution".bold());
    let task_dist = db.get_task_distribution()?;
    if task_dist.is_empty() {
        println!("  No usage data yet");
    } else {
        for dist in task_dist {
            let bar_width = (dist.percentage / 2.0) as usize;
            let bar = "█".repeat(bar_width);
            println!(
                "  {} {} ({} requests, {:.1}%)",
                dist.task_type.cyan(),
                bar.blue(),
                dist.count.to_string().yellow(),
                dist.percentage
            );
        }
    }
    println!();

    // Success rates
    println!("{}", "Success Rates".bold());
    let success_rates = db.get_success_rates()?;
    if success_rates.is_empty() {
        println!("  No usage data yet");
    } else {
        for rate in success_rates {
            let status_icon = if rate.success_rate >= 95.0 {
                "[OK]".green()
            } else if rate.success_rate >= 80.0 {
                "[WARN]".yellow()
            } else {
                "[ERROR]".red()
            };

            println!(
                "  {} {} - {:.1}% success ({}/{} requests)",
                status_icon,
                rate.service.cyan().bold(),
                rate.success_rate,
                rate.successful.to_string().green(),
                rate.total_requests.to_string().dimmed()
            );

            if rate.failed > 0 {
                println!("     {} failed requests", rate.failed.to_string().red());
            }
        }
    }
    println!();

    // Cache stats
    let (cache_entries, cache_hits, saved_tokens) = db.get_cache_stats()?;
    if cache_entries > 0 {
        println!("{}", "Cache Performance".bold());
        println!("  Cached responses: {}", cache_entries.to_string().cyan());
        println!("  Cache hits: {}", cache_hits.to_string().green());
        println!("  Tokens saved: {}", saved_tokens.to_string().yellow());
        println!();
    }

    Ok(())
}

fn show_costs(db: &Database, days: Option<i32>) -> Result<()> {
    let period = if let Some(d) = days {
        format!("Last {} Days", d)
    } else {
        "All Time".to_string()
    };

    println!(
        "\n{}",
        format!("Cost Analysis - {}", period).bold().underline()
    );
    println!();

    let cost_summaries = db.get_cost_summary(days)?;

    if cost_summaries.is_empty() {
        println!("No usage data available");
        return Ok(());
    }

    let total_cost = db.get_total_cost(days)?;

    println!("{}", "Per-Service Costs".bold());
    for summary in &cost_summaries {
        let pct = if total_cost > 0.0 {
            summary.total_cost / total_cost * 100.0
        } else {
            0.0
        };

        println!(
            "  {} - ${:.2} ({:.1}%)",
            summary.service.cyan().bold(),
            summary.total_cost,
            pct
        );
        println!(
            "     {} tokens, {} requests",
            summary.total_tokens.to_string().yellow(),
            summary.request_count.to_string().dimmed()
        );
    }
    println!();

    println!("{}", "Total Cost".bold());
    println!("  ${:.2}", total_cost);
    println!();

    // Calculate potential savings from caching
    let (_, cache_hits, saved_tokens) = db.get_cache_stats()?;
    if saved_tokens > 0 {
        // Estimate savings (using average cost per token from summaries)
        let avg_cost_per_token = if cost_summaries.iter().map(|s| s.total_tokens).sum::<i64>() > 0 {
            total_cost / cost_summaries.iter().map(|s| s.total_tokens).sum::<i64>() as f64
        } else {
            0.0
        };

        let cache_savings = saved_tokens as f64 * avg_cost_per_token;

        println!("{}", "Cache Savings".bold());
        println!(
            "  ${:.2} saved from {} cache hits",
            cache_savings, cache_hits
        );
        println!("  {} tokens avoided", saved_tokens.to_string().green());
        println!();
    }

    Ok(())
}

/// Validate an explicit CSV export destination.
///
/// Users may choose an absolute or relative path; the filesystem still
/// enforces their permissions, while this check prevents accidental writes to
/// a directory or to a non-CSV file.
fn validate_export_path(path: &str) -> Result<std::path::PathBuf> {
    use std::path::Path;

    if path.trim().is_empty() {
        return Err(anyhow::anyhow!(
            "Export path must not be empty\nExample: breadboard export --output reports/usage.csv"
        ));
    }

    let path = Path::new(path);

    if path.exists() && path.is_dir() {
        return Err(anyhow::anyhow!(
            "Export path points to a directory; choose a .csv file instead"
        ));
    }

    let is_csv = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("csv"));
    if !is_csv {
        return Err(anyhow::anyhow!(
            "Export file must have .csv extension\n\
            Example: breadboard export --output reports/usage.csv"
        ));
    }

    Ok(path.to_path_buf())
}

/// Sanitize CSV field to prevent formula injection attacks
/// Prefixes dangerous characters with single quote to prevent execution in Excel/Sheets
fn sanitize_csv_field(field: &str) -> String {
    let escaped = field.replace("\"", "\"\"");

    // Prevent formula injection by prefixing dangerous chars with single quote
    if escaped.starts_with('=')
        || escaped.starts_with('+')
        || escaped.starts_with('-')
        || escaped.starts_with('@')
        || escaped.starts_with('\t')
        || escaped.starts_with('\r')
    {
        format!("'{}", escaped)
    } else {
        escaped
    }
}

fn export_usage(db: &Database, output_path: &str) -> Result<()> {
    use std::fs::File;
    use std::io::Write;

    let history = db.get_history(10000)?; // Get all history (up to 10k entries)

    let mut file = File::create(output_path)?;

    // Write CSV header
    writeln!(
        file,
        "timestamp,service,task_type,tokens,cost_usd,exit_code,prompt_preview"
    )?;

    // Write data rows
    for entry in history {
        writeln!(
            file,
            "{},{},{},{},{:.4},{},\"{}\"",
            entry.timestamp,
            entry.service,
            entry.task_type,
            entry.tokens,
            entry.cost_usd,
            entry.exit_code,
            sanitize_csv_field(&entry.prompt_preview)
        )?;
    }

    Ok(())
}

fn show_health(db: &Database, hours: i32) -> Result<()> {
    println!(
        "\n{}",
        format!("Service Health - Last {} Hours", hours)
            .bold()
            .underline()
    );
    println!();

    let metrics = db.get_health_metrics(hours)?;

    if metrics.is_empty() {
        println!("No health data available");
        println!("Health metrics are automatically collected during command execution");
        return Ok(());
    }

    for metric in metrics {
        let health_icon = if metric.success_rate >= 95.0 && metric.avg_response_time_ms < 5000 {
            "[OK]".green()
        } else if metric.success_rate >= 80.0 && metric.avg_response_time_ms < 10000 {
            "[WARN]".yellow()
        } else {
            "[ERROR]".red()
        };

        println!("{} {}", health_icon, metric.service.cyan().bold());
        println!(
            "   Avg Response Time: {}ms",
            metric.avg_response_time_ms.to_string().yellow()
        );
        println!("   Success Rate: {:.1}%", metric.success_rate);

        if metric.recent_failures > 0 {
            println!(
                "   {} recent failures",
                metric.recent_failures.to_string().red()
            );
        }

        println!();
    }

    Ok(())
}

fn handle_profile_action(db: &Database, action: &ProfileAction) -> Result<()> {
    match action {
        ProfileAction::Create { name } => {
            db.create_profile(name)?;
            println!("{} Created profile: {}", "[OK]".green(), name.cyan());
        }
        ProfileAction::List => {
            let profiles = db.list_profiles()?;
            if profiles.is_empty() {
                println!("No profiles found");
                println!("Create one with: breadboard profile create <name>");
            } else {
                println!("\n{}", "Configuration Profiles".bold().underline());
                println!();
                for profile in profiles {
                    let indicator = if profile.active {
                        ">>".green()
                    } else {
                        " ".normal()
                    };
                    println!("  {} {}", indicator, profile.name.cyan());
                }
                println!();
            }
        }
        ProfileAction::Switch { name } => {
            db.switch_profile(name)?;
            println!("{} Switched to profile: {}", "[OK]".green(), name.cyan());
            println!("   Budgets and tiers loaded from profile");
        }
        ProfileAction::Save { name } => {
            // Create profile if it doesn't exist
            let _ = db.create_profile(name);
            db.save_current_to_profile(name)?;
            println!(
                "{} Saved current configuration to profile: {}",
                "[OK]".green(),
                name.cyan()
            );
        }
    }
    Ok(())
}

fn handle_session_action(db: &Database, action: &SessionAction) -> Result<()> {
    match action {
        SessionAction::List => sessions::list(db)?,
        SessionAction::Delete { key } => sessions::delete(db, key)?,
    }
    Ok(())
}

fn handle_bridge_action(db: &Database, config: &Config, action: &BridgeAction) -> Result<()> {
    match action {
        BridgeAction::Status => bridge::status(db)?,
        BridgeAction::Agent { action } => match action {
            AgentAction::Register {
                agent_id,
                provider,
                model,
                session,
                capabilities,
            } => bridge::register(
                db,
                agent_id,
                provider,
                model,
                session.as_deref(),
                capabilities,
            )?,
            AgentAction::List => bridge::list_agents(db)?,
            AgentAction::Unregister { agent_id } => bridge::unregister_agent(db, agent_id)?,
            AgentAction::Heartbeat { agent_id } => bridge::heartbeat(db, agent_id)?,
        },
        BridgeAction::Send {
            from,
            to,
            payload,
            message_type,
            task,
            ttl,
        } => bridge::send(db, from, to, message_type, task.as_deref(), payload, *ttl)?,
        BridgeAction::Inbox { agent, all } => bridge::inbox(db, agent, *all)?,
        BridgeAction::Ack {
            agent,
            message_id,
            claim_token,
        } => bridge::ack(db, agent, message_id, claim_token)?,
        BridgeAction::Mcp => bridge::serve_mcp(db)?,
        BridgeAction::Run {
            provider,
            model,
            session,
        } => live::run(config, db, provider, model.as_deref(), session)?,
    }
    Ok(())
}

fn handle_template_action(
    db: &Database,
    config: &Config,
    action: &TemplateAction,
    dry_run: bool,
    model_override: Option<&str>,
    session_name: Option<&str>,
    new_session: bool,
) -> Result<()> {
    match action {
        TemplateAction::Save {
            name,
            prompt,
            description,
        } => {
            // Extract variables from prompt ({{var}} format)
            let re = regex::Regex::new(r"\{\{(\w+)\}\}").unwrap();
            let variables: Vec<String> = re
                .captures_iter(prompt)
                .map(|cap| cap[1].to_string())
                .collect();

            db.save_template(name, prompt, description.as_deref(), &variables)?;
            println!("{} Saved template: {}", "[OK]".green(), name.cyan());

            if !variables.is_empty() {
                println!("   Variables: {}", variables.join(", ").yellow());
            }
        }
        TemplateAction::Use { name, vars } => {
            if let Some(template) = db.get_template(name)? {
                let mut prompt = template.prompt.clone();

                // Parse variable substitutions
                let var_map: std::collections::HashMap<String, String> = vars
                    .iter()
                    .filter_map(|v| {
                        let parts: Vec<&str> = v.splitn(2, '=').collect();
                        if parts.len() == 2 {
                            Some((parts[0].to_string(), parts[1].to_string()))
                        } else {
                            None
                        }
                    })
                    .collect();

                // Substitute variables
                for (var, value) in var_map {
                    prompt = prompt.replace(&format!("{{{{{}}}}}", var), &value);
                }

                // Check for unsubstituted variables
                let re = regex::Regex::new(r"\{\{(\w+)\}\}").unwrap();
                if let Some(cap) = re.captures(&prompt) {
                    println!("{} Missing variable: {}", "[ERROR]".red(), cap[1].yellow());
                    println!(
                        "   Usage: breadboard template use {} {}=value",
                        name, &cap[1]
                    );
                    return Ok(());
                }

                println!("{} Using template: {}", ">>".cyan(), name.cyan());
                if let Some(desc) = template.description {
                    println!("   {}", desc.dimmed());
                }
                println!();

                // Route and execute
                route_and_execute(
                    &prompt,
                    config,
                    db,
                    dry_run,
                    model_override,
                    session_name,
                    new_session,
                )?;
            } else {
                println!("{} Template not found: {}", "[ERROR]".red(), name);
            }
        }
        TemplateAction::List => {
            let templates = db.list_templates()?;
            if templates.is_empty() {
                println!("No templates found");
                println!("Create one with: breadboard template save <name> <prompt>");
            } else {
                println!("\n{}", "Prompt Templates".bold().underline());
                println!();
                for template in templates {
                    println!("{}", template.name.cyan().bold());
                    if let Some(desc) = template.description {
                        println!("   {}", desc.dimmed());
                    }
                    if !template.variables.is_empty() {
                        println!("   Variables: {}", template.variables.join(", ").yellow());
                    }
                    println!(
                        "   Preview: {}...",
                        template
                            .prompt
                            .chars()
                            .take(60)
                            .collect::<String>()
                            .dimmed()
                    );
                    println!();
                }
            }
        }
        TemplateAction::Delete { name } => {
            db.delete_template(name)?;
            println!("{} Deleted template: {}", "[OK]".green(), name.cyan());
        }
    }
    Ok(())
}

fn handle_alias_action(db: &Database, action: &AliasAction) -> Result<()> {
    match action {
        AliasAction::Set { alias, service } => {
            // Validate service
            if !constants::is_valid_service(service) {
                println!("{} Unknown service: {}", "[ERROR]".red(), service);
                if let Some(suggestion) = constants::suggest_service(service) {
                    println!("   Did you mean: {}?", suggestion.yellow());
                }
                println!(
                    "   Valid services: {}",
                    constants::SERVICES.join(", ").cyan()
                );
                return Ok(());
            }

            db.set_alias(alias, service)?;
            println!(
                "{} Set alias: {} >> {}",
                "[OK]".green(),
                alias.yellow(),
                service.cyan()
            );
        }
        AliasAction::List => {
            let aliases = db.list_aliases()?;
            if aliases.is_empty() {
                println!("No aliases defined");
                println!("Create one with: breadboard alias set <alias> <service>");
            } else {
                println!("\n{}", "Service Aliases".bold().underline());
                println!();
                for (alias, service) in aliases {
                    println!("  {} >> {}", alias.yellow(), service.cyan());
                }
                println!();
            }
        }
        AliasAction::Delete { alias } => {
            db.delete_alias(alias)?;
            println!("{} Deleted alias: {}", "[OK]".green(), alias.yellow());
        }
    }
    Ok(())
}

fn run_setup_wizard(config: &Config, db: &Database) -> Result<()> {
    use std::io::{self, Write};

    println!();
    println!("{}", "=".repeat(60).cyan());
    println!("{}", "  Breadboard Setup Wizard".bold().cyan());
    println!("{}", "=".repeat(60).cyan());
    println!();
    println!("This wizard will help you configure Breadboard for first-time use.");
    println!();

    // Step 1: Detect available CLI tools
    println!("{}", "Step 1/3: Detecting CLI Tools".bold());
    println!("{}", "-".repeat(60).dimmed());
    println!();

    let tools = vec![
        ("claude", "Claude (Anthropic)", "anthropic.com/claude-code"),
        ("codex", "Codex (OpenAI)", "platform.openai.com"),
        ("cursor", "Cursor", "cursor.sh"),
    ];

    let mut available_tools = Vec::new();

    for (cmd, name, url) in &tools {
        print!("  Checking {}... ", name);
        io::stdout().flush()?;

        if command_exists(cmd) {
            println!("{}", "[OK]".green());
            available_tools.push(*cmd);
        } else {
            println!("{}", "[NOT FOUND]".yellow());
            println!("    Install at: {}", url.dimmed());
        }
    }

    println!();

    if available_tools.is_empty() {
        println!("{}", "[WARN] No CLI tools found".yellow());
        println!();
        println!("Breadboard needs at least one AI CLI tool to function.");
        println!("Install one of the above tools and run 'breadboard setup' again.");
        println!();
        return Ok(());
    }

    println!(
        "{} Found {} tool(s): {}",
        "[OK]".green(),
        available_tools.len(),
        available_tools.join(", ").cyan()
    );
    println!();

    // Step 2: Configure subscription tiers
    println!("{}", "Step 2/3: Configure Subscription Tiers".bold());
    println!("{}", "-".repeat(60).dimmed());
    println!();

    for tool in &available_tools {
        let tier_options = match *tool {
            "claude" => vec![
                "claude-free",
                "claude-pro",
                "claude-team",
                "claude-enterprise",
            ],
            "codex" => vec!["codex-free", "codex-payg", "codex-scale"],
            "cursor" => vec!["cursor-free", "cursor-pro", "cursor-business"],
            _ => continue,
        };

        println!("  Which {} subscription do you have?", tool.cyan());
        for (i, tier) in tier_options.iter().enumerate() {
            println!("    {}. {}", i + 1, tier.yellow());
        }

        print!("  Choice [1-{}]: ", tier_options.len());
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if let Ok(choice) = input.trim().parse::<usize>() {
            if choice > 0 && choice <= tier_options.len() {
                let tier = tier_options[choice - 1];
                db.set_tier(tool, tier)?;
                println!("  {} Set {} = {}", "[OK]".green(), tool, tier.yellow());
            }
        }
        println!();
    }

    // Step 3: Test routing
    println!("{}", "Step 3/3: Test Routing".bold());
    println!("{}", "-".repeat(60).dimmed());
    println!();

    let test_prompt = "explain how async/await works";
    println!("  Testing with prompt: \"{}\"", test_prompt.dimmed());
    println!();

    let router = Router::new(config, db);
    let task_type = TaskType::classify(test_prompt);

    match router.route(task_type, None) {
        Ok(service) => {
            println!(
                "  {} Would route to: {}",
                "[OK]".green(),
                service.name().cyan()
            );
            println!("  Task type: {:?}", task_type);
        }
        Err(e) => {
            println!("  {} Routing failed: {}", "[ERROR]".red(), e);
        }
    }

    println!();
    println!("{}", "=".repeat(60).cyan());
    println!("{} Setup Complete!", "[OK]".green().bold());
    println!("{}", "=".repeat(60).cyan());
    println!();
    println!("Next steps:");
    println!("  breadboard status              # Check usage");
    println!("  breadboard --dry-run \"test\"    # Test routing");
    println!("  breadboard \"your prompt here\"  # Start using");
    println!();

    Ok(())
}

fn run_doctor(config: &Config, db: &Database) -> Result<()> {
    println!();
    println!("{}", "Breadboard Health Check".bold().underline());
    println!();

    let mut issues = Vec::new();
    let mut warnings = Vec::new();

    // Check binary installation
    println!("{}", "System:".bold());
    if let Ok(exe) = std::env::current_exe() {
        println!(
            "  {} Binary: {}",
            "[OK]".green(),
            exe.display().to_string().dimmed()
        );
    }

    // Check database
    let db_path = dirs::data_local_dir().map(|mut p| {
        p.push("breadboard");
        p.push("usage.db");
        p
    });
    if let Some(path) = db_path {
        if path.exists() {
            println!(
                "  {} Database: {}",
                "[OK]".green(),
                path.display().to_string().dimmed()
            );
        } else {
            println!(
                "  {} Database: not found (will be created)",
                "[WARN]".yellow()
            );
        }
    }

    // Check config
    let config_path = dirs::config_dir().map(|mut p| {
        p.push("breadboard");
        p.push("config.yaml");
        p
    });
    if let Some(path) = config_path {
        if path.exists() {
            println!(
                "  {} Config: {}",
                "[OK]".green(),
                path.display().to_string().dimmed()
            );
        } else {
            println!("  {} Config: using defaults", "[INFO]".cyan());
        }
    }

    println!();
    println!("{}", "Services:".bold());

    let services = vec![
        (
            "claude",
            config.claude_command.as_deref().unwrap_or("claude"),
        ),
        ("codex", config.codex_command.as_deref().unwrap_or("codex")),
        (
            "cursor",
            config.cursor_command.as_deref().unwrap_or("cursor"),
        ),
    ];

    for (name, cmd) in &services {
        let tier = db.get_tier(name)?;
        let budget = db.get_budget(name)?;
        let usage = db.get_usage(name)?;

        if command_exists(cmd) {
            let status = if tier.is_some() {
                "[OK]".green()
            } else {
                warnings.push(format!("{} is installed but no tier configured", name));
                "[WARN]".yellow()
            };

            println!("  {} {} (command: {})", status, name.cyan(), cmd.dimmed());

            if let Some(t) = tier {
                println!("     Tier: {}", t.yellow());
                println!(
                    "     Usage: {}/{} tokens ({:.1}%)",
                    usage,
                    budget,
                    if budget > 0 {
                        usage as f64 / budget as f64 * 100.0
                    } else {
                        0.0
                    }
                );
            } else {
                println!("     Run: breadboard set-tier {} <tier>", name);
            }
        } else if tier.is_some() {
            issues.push(format!(
                "{} is configured but command '{}' not found",
                name, cmd
            ));
            println!(
                "  {} {} - command '{}' not found",
                "[ERROR]".red(),
                name.cyan(),
                cmd
            );
        } else {
            println!("  {} {} - not installed", "[INFO]".dimmed(), name.dimmed());
        }
    }

    println!();

    // Summary
    if issues.is_empty() && warnings.is_empty() {
        println!("{} All systems operational!", "[OK]".green().bold());
    } else {
        if !warnings.is_empty() {
            println!("{}", "Warnings:".yellow().bold());
            for warning in &warnings {
                println!("  - {}", warning);
            }
            println!();
        }

        if !issues.is_empty() {
            println!("{}", "Issues:".red().bold());
            for issue in &issues {
                println!("  - {}", issue);
            }
            println!();
            println!("Run 'breadboard setup' to fix configuration issues.");
        }
    }

    println!();

    Ok(())
}

fn command_exists(cmd: &str) -> bool {
    use std::process::Command;
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::validate_export_path;

    #[test]
    fn export_accepts_relative_and_absolute_csv_paths() {
        assert!(validate_export_path("reports/usage.csv").is_ok());
        assert!(validate_export_path("/tmp/usage.CSV").is_ok());
    }

    #[test]
    fn export_rejects_empty_and_non_csv_paths() {
        assert!(validate_export_path("").is_err());
        assert!(validate_export_path("usage.txt").is_err());
    }
}
