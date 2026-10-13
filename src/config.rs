use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub claude_command: Option<String>,
    pub codex_command: Option<String>,
    pub cursor_command: Option<String>,

    /// Compress command outputs when storing in history (reduces DB size)
    #[serde(default = "default_compress_history")]
    pub compress_history: bool,

    /// Default and task-specific model IDs by service.
    #[serde(default)]
    pub models: HashMap<String, HashMap<String, String>>,
}

fn default_compress_history() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Config {
            claude_command: Some("claude".to_string()),
            codex_command: Some("codex".to_string()),
            cursor_command: Some("cursor".to_string()),
            compress_history: true,
            models: HashMap::new(),
        }
    }
}

impl Config {
    pub fn model_for(&self, service: &str, task_type: &str) -> Option<String> {
        let service_models = self.models.get(service)?;
        service_models
            .get(task_type)
            .or_else(|| service_models.get("default"))
            .cloned()
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;

        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let config: Config = serde_yaml::from_str(&content)?;
            Ok(config)
        } else {
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
            restrict_permissions(parent, 0o700)?;
        }
        let content = serde_yaml::to_string(self)?;
        fs::write(&path, content)?;
        restrict_permissions(&path, 0o600)?;
        Ok(())
    }

    fn config_path() -> Result<PathBuf> {
        let mut path =
            dirs::config_dir().ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?;
        path.push("breadboard");
        fs::create_dir_all(&path)?;
        restrict_permissions(&path, 0o700)?;
        path.push("config.yaml");
        Ok(path)
    }
}

#[cfg(unix)]
fn restrict_permissions(path: &std::path::Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(mode);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &std::path::Path, _mode: u32) -> Result<()> {
    Ok(())
}

pub fn interactive_config() -> Result<()> {
    use std::io::{self, Write};

    println!("Breadboard Configuration");
    println!("==========================");
    println!();

    let mut config = Config::load()?;

    println!("Enter the command to call each service (press Enter to keep current):");
    println!();

    print!(
        "Claude command [{}]: ",
        config.claude_command.as_ref().unwrap_or(&"".to_string())
    );
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    if !input.trim().is_empty() {
        config.claude_command = Some(input.trim().to_string());
    }

    print!(
        "Codex command [{}]: ",
        config.codex_command.as_ref().unwrap_or(&"".to_string())
    );
    io::stdout().flush()?;
    input.clear();
    io::stdin().read_line(&mut input)?;
    if !input.trim().is_empty() {
        config.codex_command = Some(input.trim().to_string());
    }

    print!(
        "Cursor command [{}]: ",
        config.cursor_command.as_ref().unwrap_or(&"".to_string())
    );
    io::stdout().flush()?;
    input.clear();
    io::stdin().read_line(&mut input)?;
    if !input.trim().is_empty() {
        config.cursor_command = Some(input.trim().to_string());
    }

    println!();
    println!("Enter default model IDs (press Enter to keep current or leave blank):");
    for service in ["claude", "codex", "cursor"] {
        let current = config
            .models
            .get(service)
            .and_then(|models| models.get("default"))
            .map(String::as_str)
            .unwrap_or("");

        print!("{} model [{}]: ", service, current);
        io::stdout().flush()?;
        input.clear();
        io::stdin().read_line(&mut input)?;
        if !input.trim().is_empty() {
            config
                .models
                .entry(service.to_string())
                .or_default()
                .insert("default".to_string(), input.trim().to_string());
        }
    }

    println!("Task-specific model overrides can be added to config.yaml manually.");

    config.save()?;

    println!();
    println!("[OK] Configuration saved!");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_model_overrides_service_default() {
        let mut config = Config::default();
        let mut models = HashMap::new();
        models.insert("default".to_string(), "sonnet".to_string());
        models.insert("code_generation".to_string(), "opus".to_string());
        config.models.insert("claude".to_string(), models);

        assert_eq!(
            config.model_for("claude", "code_generation"),
            Some("opus".to_string())
        );
        assert_eq!(
            config.model_for("claude", "chat"),
            Some("sonnet".to_string())
        );
        assert_eq!(config.model_for("codex", "chat"), None);
    }
}
