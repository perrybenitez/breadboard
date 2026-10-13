use anyhow::{anyhow, Result};
use std::path::Path;

use crate::db::Database;

#[derive(Debug, Clone)]
pub struct SessionContext {
    pub name: String,
    pub key: String,
    pub service: String,
    pub model_id: String,
    pub project_root: String,
    pub provider_session_id: Option<String>,
    pub is_new: bool,
}

/// Resolve a user-facing session name to a stable Breadboard session identity.
pub fn open(
    db: &Database,
    service: &str,
    model_id: Option<&str>,
    name: &str,
    force_new: bool,
    persist: bool,
) -> Result<SessionContext> {
    validate_name(name)?;

    let project_root = project_root()?;
    let model_id = model_id.unwrap_or("default");
    let key = session_key(service, model_id, &project_root, name);
    let existing = db.get_session(&key)?;

    if force_new && existing.is_some() {
        return Err(anyhow!(
            "session '{}' already exists for this provider/model/project; choose a new name or delete session {}",
            name,
            key
        ));
    }

    let provider_session_id = existing
        .as_ref()
        .and_then(|session| session.provider_session_id.clone())
        .or_else(|| (service == "claude").then(|| claude_session_id(&key)));

    if persist && existing.is_none() {
        db.create_session(
            &key,
            name,
            service,
            model_id,
            &project_root,
            provider_session_id.as_deref(),
        )?;
    } else if persist {
        if let Some(provider_session_id) = &provider_session_id {
            db.set_provider_session_id(&key, provider_session_id)?;
        }
    }

    Ok(SessionContext {
        name: name.to_string(),
        key,
        service: service.to_string(),
        model_id: model_id.to_string(),
        project_root,
        provider_session_id,
        is_new: existing.is_none(),
    })
}

pub fn print_started(context: &SessionContext) {
    let state = if context.is_new { "new" } else { "resuming" };
    println!("Session: {} ({})", context.name, state);
    println!(
        "Provider: {}  Model: {}  Project: {}",
        context.service, context.model_id, context.project_root
    );
    println!("Session key: {}", context.key);
    println!("  The key is opaque; your prompt is not included in it.");
    println!(
        "Resume with: breadboard --session {} \"your prompt\"",
        context.name
    );
}

pub fn print_finished(context: &SessionContext) {
    println!("Session closed: {}", context.key);
}

pub fn list(db: &Database) -> Result<()> {
    let sessions = db.list_sessions()?;
    if sessions.is_empty() {
        println!("No Breadboard sessions found.");
        return Ok(());
    }

    println!("Breadboard sessions:");
    for session in sessions {
        println!(
            "  {}  {}  {}  {}",
            session.session_name, session.service, session.model_id, session.session_key
        );
        println!("     project: {}", session.project_root);
        println!(
            "     status: {}  created: {}",
            session.status, session.created_at
        );
        println!("     last used: {}", session.last_used_at);
    }
    Ok(())
}

pub fn delete(db: &Database, key: &str) -> Result<()> {
    if db.delete_session(key)? {
        println!("Deleted session {}", key);
    } else {
        println!("No session found for key {}", key);
    }
    Ok(())
}

fn project_root() -> Result<String> {
    let current = std::env::current_dir()?;
    let root = find_project_root(&current).unwrap_or(current);
    Ok(root
        .canonicalize()
        .unwrap_or(root)
        .to_string_lossy()
        .into_owned())
}

fn find_project_root(path: &Path) -> Option<std::path::PathBuf> {
    let mut current = Some(path);
    while let Some(candidate) = current {
        if candidate.join(".git").exists() {
            return Some(candidate.to_path_buf());
        }
        current = candidate.parent();
    }
    None
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() || name.len() > 64 {
        return Err(anyhow!("session name must be between 1 and 64 characters"));
    }
    if !name
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'))
    {
        return Err(anyhow!(
            "session name may contain only letters, numbers, '-', '_', and '.'"
        ));
    }
    Ok(())
}

fn session_key(service: &str, model_id: &str, project_root: &str, name: &str) -> String {
    let canonical = format!("{}\0{}\0{}\0{}", service, model_id, project_root, name);
    let digest = sha256(canonical.as_bytes());
    format!("bb_{}", hex_encode(&digest))
}

fn claude_session_id(key: &str) -> String {
    let digest = sha256(key.as_bytes());
    let hex = hex_encode(&digest);
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

// Self-contained SHA-256 implementation to keep Breadboard usable offline.
fn sha256(input: &[u8]) -> [u8; 32] {
    const INITIAL: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    const ROUND: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let bit_length = (input.len() as u64) * 8;
    let padded_len = (input.len() + 9).div_ceil(64) * 64;
    let mut padded = vec![0u8; padded_len];
    padded[..input.len()].copy_from_slice(input);
    padded[input.len()] = 0x80;
    padded[padded_len - 8..].copy_from_slice(&bit_length.to_be_bytes());

    let mut state = INITIAL;
    for chunk in padded.chunks_exact(64) {
        let mut words = [0u32; 64];
        for (index, word) in words[..16].iter_mut().enumerate() {
            let offset = index * 4;
            *word = u32::from_be_bytes([
                chunk[offset],
                chunk[offset + 1],
                chunk[offset + 2],
                chunk[offset + 3],
            ]);
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }

        let mut working = state;
        for index in 0..64 {
            let ch = (working[4] & working[5]) ^ ((!working[4]) & working[6]);
            let maj =
                (working[0] & working[1]) ^ (working[0] & working[2]) ^ (working[1] & working[2]);
            let sum0 = working[0].rotate_right(2)
                ^ working[0].rotate_right(13)
                ^ working[0].rotate_right(22);
            let sum1 = working[4].rotate_right(6)
                ^ working[4].rotate_right(11)
                ^ working[4].rotate_right(25);
            let temp1 = working[7]
                .wrapping_add(sum1)
                .wrapping_add(ch)
                .wrapping_add(ROUND[index])
                .wrapping_add(words[index]);
            let temp2 = sum0.wrapping_add(maj);

            working[7] = working[6];
            working[6] = working[5];
            working[5] = working[4];
            working[4] = working[3].wrapping_add(temp1);
            working[3] = working[2];
            working[2] = working[1];
            working[1] = working[0];
            working[0] = temp1.wrapping_add(temp2);
        }

        for index in 0..8 {
            state[index] = state[index].wrapping_add(working[index]);
        }
    }

    let mut output = [0u8; 32];
    for (index, word) in state.iter().enumerate() {
        output[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_keys_are_stable_and_scoped() {
        let one = session_key("claude", "sonnet", "/repo", "work");
        let same = session_key("claude", "sonnet", "/repo", "work");
        let different_model = session_key("claude", "opus", "/repo", "work");

        assert_eq!(one, same);
        assert_ne!(one, different_model);
        assert!(one.starts_with("bb_"));
    }

    #[test]
    fn claude_session_id_is_uuid_shaped() {
        let id = claude_session_id("bb_test");
        assert_eq!(id.len(), 36);
        assert_eq!(id.chars().filter(|character| *character == '-').count(), 4);
    }

    #[test]
    fn sha256_matches_known_digest() {
        assert_eq!(
            hex_encode(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
