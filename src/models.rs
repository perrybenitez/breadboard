use anyhow::{anyhow, Context, Result};
use colored::*;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::constants;

const COMMAND_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelInfo {
    pub id: String,
    pub name: Option<String>,
}

#[derive(Debug)]
struct Discovery {
    models: Vec<ModelInfo>,
    note: Option<String>,
    native_output: Option<String>,
}

/// Discover model information through each provider's native CLI/API surface.
pub fn run(config: &Config, requested_service: Option<&str>) -> Result<()> {
    let services: Vec<&str> = match requested_service {
        Some(service) => {
            if !constants::is_valid_service(service) {
                return Err(anyhow!(
                    "unknown service '{}'; valid services: {}",
                    service,
                    constants::SERVICES.join(", ")
                ));
            }
            vec![service]
        }
        None => constants::SERVICES.to_vec(),
    };

    for (index, service) in services.iter().enumerate() {
        if index > 0 {
            println!();
        }

        println!("{} {}", "Models for".bold(), service.cyan().bold());
        match discover(config, service) {
            Ok(discovery) => print_discovery(&discovery),
            Err(error) if requested_service.is_none() => {
                println!("{} {}", "[WARN]".yellow(), error);
                println!("  The other providers can still be inspected independently.");
            }
            Err(error) => return Err(error),
        }
    }

    if requested_service.is_none() {
        println!();
        println!(
            "Use `breadboard models <service>` to inspect one provider, then set an ID with `--model` or in config.yaml."
        );
    }

    Ok(())
}

fn print_discovery(discovery: &Discovery) {
    if let Some(note) = &discovery.note {
        println!("{}", note.yellow());
    }

    if !discovery.models.is_empty() {
        println!("{}", "Discovered model IDs:".bold());
        for model in &discovery.models {
            match &model.name {
                Some(name) if name != &model.id => println!("  {} ({})", model.id.green(), name),
                _ => println!("  {}", model.id.green()),
            }
        }
    }

    if let Some(output) = &discovery.native_output {
        println!();
        println!("{}", "Provider output:".bold());
        let trimmed = output.trim();
        if !trimmed.is_empty() {
            println!("{}", trimmed);
        }
    }

    if discovery.models.is_empty() && discovery.native_output.is_none() {
        println!("No model IDs were returned.");
    }
}

fn discover(config: &Config, service: &str) -> Result<Discovery> {
    let command = configured_command(config, service);

    match service {
        "claude" => discover_claude(&command),
        "codex" => discover_codex(&command),
        "cursor" => discover_cursor(&command),
        _ => Err(anyhow!("unsupported service: {}", service)),
    }
}

fn configured_command(config: &Config, service: &str) -> String {
    match service {
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
        _ => service.to_string(),
    }
}

fn discover_claude(command: &str) -> Result<Discovery> {
    let output = run_command(command, &["--help"])
        .with_context(|| format!("could not run '{}' to inspect model support", command))?;

    if !output.success {
        return Err(anyhow!(
            "'{} --help' failed{}",
            command,
            format_exit_detail(&output)
        ));
    }

    let model_help = output
        .stdout
        .lines()
        .filter(|line| line.to_ascii_lowercase().contains("model"))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(Discovery {
        models: Vec::new(),
        note: Some(
            "This CLI does not expose an account-specific non-interactive model catalog; use its model picker for availability, then pass the exact ID here.".to_string(),
        ),
        native_output: (!model_help.is_empty()).then_some(model_help),
    })
}

fn discover_cursor(command: &str) -> Result<Discovery> {
    let mut candidates = vec![command.to_string()];
    if command == "cursor" {
        // Current Cursor CLI installations commonly expose `cursor-agent`.
        candidates.push("cursor-agent".to_string());
    }

    let mut last_error = None;
    for candidate in candidates {
        match run_command(&candidate, &["models"]) {
            Ok(output) if output.success => {
                let native_output = if output.stdout.trim().is_empty() {
                    output.stderr
                } else {
                    output.stdout
                };
                return Ok(Discovery {
                    models: parse_cursor_models(&native_output),
                    note: None,
                    native_output: Some(native_output),
                });
            }
            Ok(output) => {
                last_error = Some(format!(
                    "'{} models' failed{}",
                    candidate,
                    format_exit_detail(&output)
                ));
            }
            Err(error) => last_error = Some(error.to_string()),
        }
    }

    Err(anyhow!(
        "Cursor model discovery failed: {}",
        last_error.unwrap_or_else(|| "no Cursor command found".to_string())
    ))
}

fn discover_codex(command: &str) -> Result<Discovery> {
    let mut child = Command::new(command)
        .args(["app-server", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("could not start '{}' app-server", command))?;

    let result = (|| -> Result<Discovery> {
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Codex app-server did not expose stdout"))?;
        let response_lines = spawn_line_reader(stdout);
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Codex app-server did not expose stdin"))?;

        write_jsonrpc(
            &mut stdin,
            1,
            "initialize",
            json!({
                "clientInfo": {
                    "name": "breadboard",
                    "title": "Breadboard model discovery",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {
                    "experimentalApi": false
                }
            }),
        )?;
        wait_for_response(&response_lines, 1)?;

        writeln!(
            stdin,
            "{}",
            json!({
                "jsonrpc": "2.0",
                "method": "initialized",
                "params": {}
            })
        )?;
        stdin.flush()?;

        write_jsonrpc(
            &mut stdin,
            2,
            "model/list",
            json!({
                "limit": 100,
                "cursor": Value::Null,
                "includeHidden": false
            }),
        )?;
        let result = wait_for_response(&response_lines, 2)?;
        let models = parse_model_catalog(&result);

        Ok(Discovery {
            models,
            note: None,
            native_output: None,
        })
    })();

    let _ = child.kill();
    let _ = child.wait();
    result
}

fn write_jsonrpc<W: Write>(writer: &mut W, id: u64, method: &str, params: Value) -> Result<()> {
    writeln!(
        writer,
        "{}",
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        })
    )?;
    writer.flush()?;
    Ok(())
}

fn spawn_line_reader(
    stdout: impl std::io::Read + Send + 'static,
) -> Receiver<Result<String, String>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let message = line.map_err(|error| error.to_string());
            if sender.send(message).is_err() {
                break;
            }
        }
    });
    receiver
}

fn wait_for_response(receiver: &Receiver<Result<String, String>>, id: u64) -> Result<Value> {
    let deadline = Instant::now() + COMMAND_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(anyhow!("timed out waiting for Codex app-server response"));
        }

        let line = receiver
            .recv_timeout(remaining)
            .map_err(|_| anyhow!("timed out waiting for Codex app-server response"))?
            .map_err(|error| anyhow!("Codex app-server output error: {}", error))?;
        let message: Value = match serde_json::from_str(&line) {
            Ok(message) => message,
            Err(_) => continue,
        };

        if message.get("id").and_then(Value::as_u64) != Some(id) {
            continue;
        }

        if let Some(error) = message.get("error") {
            return Err(anyhow!("Codex app-server error: {}", error));
        }

        return Ok(message.get("result").cloned().unwrap_or(Value::Null));
    }
}

fn parse_model_catalog(value: &Value) -> Vec<ModelInfo> {
    let mut models = Vec::new();
    collect_model_info(value, &mut models);
    models.sort_by(|left, right| left.id.cmp(&right.id));
    models
}

fn collect_model_info(value: &Value, models: &mut Vec<ModelInfo>) {
    match value {
        Value::Object(object) => {
            let id = object
                .get("slug")
                .or_else(|| object.get("model"))
                .and_then(Value::as_str)
                .map(str::to_string);
            let name = object
                .get("displayName")
                .or_else(|| object.get("display_name"))
                .or_else(|| object.get("name"))
                .and_then(Value::as_str)
                .map(str::to_string);

            if let Some(id) = id {
                if object.contains_key("slug") || name.is_some() {
                    if !models.iter().any(|model| model.id == id) {
                        models.push(ModelInfo { id, name });
                    }
                }
            }

            for child in object.values() {
                collect_model_info(child, models);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_model_info(value, models);
            }
        }
        _ => {}
    }
}

fn parse_cursor_models(output: &str) -> Vec<ModelInfo> {
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty()
                || line.starts_with('-')
                || line.to_ascii_lowercase().contains("model")
            {
                return None;
            }

            let id = line
                .split_whitespace()
                .next()?
                .trim_matches(|character: char| character == '|' || character == '`');
            if id.is_empty() || id.len() > 200 {
                return None;
            }

            Some(ModelInfo {
                id: id.to_string(),
                name: None,
            })
        })
        .collect()
}

struct CommandOutput {
    success: bool,
    stdout: String,
    stderr: String,
    status: String,
}

fn run_command(command: &str, args: &[&str]) -> Result<CommandOutput> {
    let mut child = Command::new(command)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("could not start '{}'", command))?;

    let deadline = Instant::now() + COMMAND_TIMEOUT;
    loop {
        if child.try_wait()?.is_some() {
            let output = child.wait_with_output()?;
            return Ok(CommandOutput {
                success: output.status.success(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                status: output.status.to_string(),
            });
        }

        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(anyhow!(
                "'{}' timed out after {} seconds",
                command,
                COMMAND_TIMEOUT.as_secs()
            ));
        }

        thread::sleep(Duration::from_millis(50));
    }
}

fn format_exit_detail(output: &CommandOutput) -> String {
    let detail = if output.stderr.trim().is_empty() {
        output.stdout.trim()
    } else {
        output.stderr.trim()
    };

    if detail.is_empty() {
        format!(" (exit status {})", output.status)
    } else {
        format!(" ({}; {})", output.status, detail)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_codex_model_catalog() {
        let value = json!({
            "data": [
                {"slug": "gpt-5-codex", "displayName": "GPT-5-Codex"},
                {"slug": "o4-mini", "display_name": "o4-mini"}
            ]
        });

        assert_eq!(
            parse_model_catalog(&value),
            vec![
                ModelInfo {
                    id: "gpt-5-codex".to_string(),
                    name: Some("GPT-5-Codex".to_string())
                },
                ModelInfo {
                    id: "o4-mini".to_string(),
                    name: Some("o4-mini".to_string())
                },
            ]
        );
    }

    #[test]
    fn parses_cursor_text_without_headers() {
        let models = parse_cursor_models("Available models\n-----------------\nsonnet\nopus\n");
        assert_eq!(models[0].id, "sonnet");
        assert_eq!(models[1].id, "opus");
    }
}
