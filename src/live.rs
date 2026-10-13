use anyhow::{anyhow, Context, Result};
use serde_json::{json, Map, Value};
use std::io::{self, BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use crate::bridge;
use crate::config::Config;
use crate::constants;
use crate::db::Database;
use crate::sessions::{self, SessionContext};

const POLL_INTERVAL: Duration = Duration::from_millis(250);
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5 * 60);

pub fn run(
    config: &Config,
    db: &Database,
    provider: &str,
    model: Option<&str>,
    session_name: &str,
) -> Result<()> {
    if !constants::is_valid_service(provider) {
        return Err(anyhow!(
            "unknown provider '{}'; valid providers: {}",
            provider,
            constants::SERVICES.join(", ")
        ));
    }
    if provider == "cursor" {
        return Err(anyhow!(
            "Cursor does not currently expose a supported bidirectional live transport; use a normal resumed session for queued delivery"
        ));
    }

    let session = sessions::open(db, provider, model, session_name, false, true)?;
    let agent_id = bridge::register_session_agent(db, &session, "live-session")?;
    let mut transport = match LiveTransport::start(config, &session) {
        Ok(transport) => transport,
        Err(error) => {
            let _ = db.mark_agent_offline(&agent_id);
            return Err(error);
        }
    };

    if let Some(provider_session_id) = transport.provider_session_id() {
        if session.provider_session_id.as_deref() != Some(provider_session_id) {
            db.set_provider_session_id(&session.key, provider_session_id)?;
        }
    }

    println!("Live session: {}", session.name);
    println!("Agent: {}", agent_id);
    println!("Provider: {}  Model: {}", provider, session.model_id);
    println!("Enter prompts; bridge messages are delivered between turns.");
    println!("Press Ctrl-D to close the live session.");

    let input = spawn_stdin_reader();
    let result = run_turn_loop(db, &agent_id, provider, &mut transport, input);
    let _ = db.mark_agent_offline(&agent_id);
    transport.close();
    result
}

trait TurnTransport {
    fn send_turn(&mut self, prompt: &str) -> Result<TurnResult>;
    fn close(&mut self);
}

impl TurnTransport for LiveTransport {
    fn send_turn(&mut self, prompt: &str) -> Result<TurnResult> {
        LiveTransport::send_turn(self, prompt)
    }

    fn close(&mut self) {
        LiveTransport::close(self);
    }
}

fn run_turn_loop<T: TurnTransport>(
    db: &Database,
    agent_id: &str,
    provider: &str,
    transport: &mut T,
    input: Receiver<Option<String>>,
) -> Result<()> {
    let mut last_heartbeat = Instant::now();
    loop {
        if last_heartbeat.elapsed() >= HEARTBEAT_INTERVAL {
            db.touch_agent(agent_id)?;
            last_heartbeat = Instant::now();
        }

        // A live session receives bridge work without waiting for another
        // human prompt, but only at the boundary before the next turn.
        let (bridge_prompt, bridge_messages) = bridge::inject_pending_messages(db, agent_id, "")?;
        if !bridge_messages.is_empty() {
            process_turn(
                db,
                agent_id,
                provider,
                transport,
                &bridge_prompt,
                bridge_messages,
            )?;
            continue;
        }

        match input.recv_timeout(POLL_INTERVAL) {
            Ok(Some(prompt)) if !prompt.trim().is_empty() => {
                let (prompt, bridge_messages) =
                    bridge::inject_pending_messages(db, agent_id, &prompt)?;
                process_turn(db, agent_id, provider, transport, &prompt, bridge_messages)?;
            }
            Ok(Some(_)) => {}
            Ok(None) => return Ok(()),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
        }
    }
}

fn process_turn<T: TurnTransport>(
    db: &Database,
    agent_id: &str,
    provider: &str,
    transport: &mut T,
    prompt: &str,
    bridge_messages: Vec<crate::db::BridgeMessage>,
) -> Result<()> {
    let budget = db.get_budget(provider)?;
    let usage = db.get_usage(provider)?;
    if budget > 0 && usage >= budget {
        return Err(anyhow!(
            "{} live session stopped at its monthly budget ({}/{} tokens)",
            provider,
            usage,
            budget
        ));
    }

    let result = transport.send_turn(prompt)?;
    if !result.output.trim().is_empty() {
        println!("\n{}\n", result.output.trim());
    }

    let estimated_tokens = ((prompt.len() + result.output.len()) as i64 / 4).max(1);
    let tokens = result.tokens_used.unwrap_or(estimated_tokens);
    db.record_usage(provider, tokens, prompt, 0, 0.0, "live_session")?;

    // Usage is recorded before acknowledgment so a local database failure
    // cannot silently lose accounting for a provider turn. Delivery remains
    // at-least-once: if acknowledgment fails, the lease will make the bridge
    // message retryable after the provider turn has already completed.
    if !bridge_messages.is_empty() {
        bridge::acknowledge_messages(db, agent_id, &bridge_messages)?;
    }

    db.touch_agent(agent_id)?;
    Ok(())
}

fn spawn_stdin_reader() -> Receiver<Option<String>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            match line {
                Ok(line) => {
                    if sender.send(Some(line)).is_err() {
                        return;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = sender.send(None);
    });
    receiver
}

#[derive(Debug, PartialEq)]
struct TurnResult {
    output: String,
    tokens_used: Option<i64>,
}

enum LiveTransport {
    Claude(ClaudeTransport),
    Codex(CodexTransport),
}

impl LiveTransport {
    fn start(config: &Config, session: &SessionContext) -> Result<Self> {
        match session.service.as_str() {
            "claude" => Ok(Self::Claude(ClaudeTransport::start(config, session)?)),
            "codex" => Ok(Self::Codex(CodexTransport::start(config, session)?)),
            provider => Err(anyhow!(
                "live transport is not implemented for {}",
                provider
            )),
        }
    }

    fn provider_session_id(&self) -> Option<&str> {
        match self {
            Self::Claude(_) => None,
            Self::Codex(transport) => Some(&transport.thread_id),
        }
    }

    fn send_turn(&mut self, prompt: &str) -> Result<TurnResult> {
        match self {
            Self::Claude(transport) => transport.send_turn(prompt),
            Self::Codex(transport) => transport.send_turn(prompt),
        }
    }

    fn close(&mut self) {
        match self {
            Self::Claude(transport) => transport.close(),
            Self::Codex(transport) => transport.close(),
        }
    }
}

struct ClaudeTransport {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl ClaudeTransport {
    fn start(config: &Config, session: &SessionContext) -> Result<Self> {
        let command = config.claude_command.as_deref().unwrap_or("claude");
        let mut args = vec![
            "-p".to_string(),
            "--input-format".to_string(),
            "stream-json".to_string(),
            "--output-format".to_string(),
            "stream-json".to_string(),
        ];
        if session.model_id != "default" {
            args.push("--model".to_string());
            args.push(session.model_id.clone());
        }
        if let Some(provider_session_id) = &session.provider_session_id {
            if session.is_new {
                args.push("--session-id".to_string());
            } else {
                args.push("--resume".to_string());
            }
            args.push(provider_session_id.clone());
        }

        let mut child = Command::new(command)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("could not start '{}' live session", command))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Claude live session did not expose stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Claude live session did not expose stdout"))?;
        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        })
    }

    fn send_turn(&mut self, prompt: &str) -> Result<TurnResult> {
        writeln!(
            self.stdin,
            "{}",
            json!({
                "type": "user",
                "message": {
                    "role": "user",
                    "content": [{"type": "text", "text": prompt}]
                }
            })
        )?;
        self.stdin.flush()?;

        let mut output = String::new();
        let mut line = String::new();
        loop {
            line.clear();
            if self.stdout.read_line(&mut line)? == 0 {
                return Err(anyhow!("Claude live session closed before turn completion"));
            }
            let event: Value = match serde_json::from_str(line.trim()) {
                Ok(event) => event,
                Err(_) => continue,
            };
            if let Some(result) = process_claude_event(&event, &mut output)? {
                return Ok(result);
            }
        }
    }

    fn close(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn process_claude_event(event: &Value, output: &mut String) -> Result<Option<TurnResult>> {
    match event.get("type").and_then(Value::as_str).unwrap_or("") {
        "assistant" => append_claude_content(output, event),
        "content_block_delta" => {
            if let Some(text) = event
                .get("delta")
                .and_then(|delta| delta.get("text"))
                .and_then(Value::as_str)
            {
                output.push_str(text);
            }
        }
        "result" => {
            if event.get("is_error").and_then(Value::as_bool) == Some(true) {
                return Err(anyhow!(
                    "Claude live turn failed: {}",
                    event
                        .get("result")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown error")
                ));
            }
            if output.is_empty() {
                if let Some(text) = event.get("result").and_then(Value::as_str) {
                    output.push_str(text);
                }
            }
            return Ok(Some(TurnResult {
                output: std::mem::take(output),
                tokens_used: usage_tokens(event.get("usage")),
            }));
        }
        _ => {}
    }
    Ok(None)
}

fn append_claude_content(output: &mut String, event: &Value) {
    let Some(content) = event
        .get("message")
        .and_then(|message| message.get("content"))
        .and_then(Value::as_array)
    else {
        return;
    };
    for block in content {
        if block.get("type").and_then(Value::as_str) == Some("text") {
            if let Some(text) = block.get("text").and_then(Value::as_str) {
                output.push_str(text);
            }
        }
    }
}

struct CodexTransport {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    thread_id: String,
}

impl CodexTransport {
    fn start(config: &Config, session: &SessionContext) -> Result<Self> {
        let command = config.codex_command.as_deref().unwrap_or("codex");
        let mut child = Command::new(command)
            .args(["app-server", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("could not start '{}' live session", command))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Codex live session did not expose stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Codex live session did not expose stdout"))?;
        let mut transport = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
            thread_id: String::new(),
        };
        transport.request(
            "initialize",
            json!({
                "clientInfo": {
                    "name": "breadboard",
                    "title": "Breadboard live session",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {"experimentalApi": false}
            }),
        )?;
        transport.notify("initialized", json!({}))?;

        let (method, params) = if let Some(thread_id) = &session.provider_session_id {
            ("thread/resume", json!({"threadId": thread_id}))
        } else {
            let mut params = Map::new();
            params.insert(
                "cwd".to_string(),
                Value::String(session.project_root.clone()),
            );
            if session.model_id != "default" {
                params.insert("model".to_string(), Value::String(session.model_id.clone()));
            }
            ("thread/start", Value::Object(params))
        };
        let result = transport.request(method, params)?;
        transport.thread_id = nested_string(&result, &["thread", "id"])
            .or_else(|| {
                result
                    .get("threadId")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .ok_or_else(|| anyhow!("Codex app-server did not return a thread ID"))?;
        Ok(transport)
    }

    fn send_turn(&mut self, prompt: &str) -> Result<TurnResult> {
        let request_id = self.next_id;
        self.next_id += 1;
        write_jsonrpc(
            &mut self.stdin,
            request_id,
            "turn/start",
            json!({
                "threadId": self.thread_id,
                "input": [{"type": "text", "text": prompt}]
            }),
        )?;

        let mut output = String::new();
        let mut response_seen = false;
        let mut line = String::new();
        loop {
            line.clear();
            if self.stdout.read_line(&mut line)? == 0 {
                return Err(anyhow!("Codex live session closed before turn completion"));
            }
            let event: Value = match serde_json::from_str(line.trim()) {
                Ok(event) => event,
                Err(_) => continue,
            };
            if event.get("id").and_then(Value::as_u64) == Some(request_id) {
                response_seen = true;
                if let Some(error) = event.get("error") {
                    return Err(anyhow!("Codex turn failed: {}", error));
                }
                continue;
            }
            if let Some(result) = process_codex_event(
                &event,
                request_id,
                &mut response_seen,
                &mut output,
            )? {
                return Ok(result);
            }
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        write_jsonrpc(&mut self.stdin, id, method, params)?;
        let mut line = String::new();
        loop {
            line.clear();
            if self.stdout.read_line(&mut line)? == 0 {
                return Err(anyhow!("Codex app-server closed during {}", method));
            }
            let response: Value = match serde_json::from_str(line.trim()) {
                Ok(response) => response,
                Err(_) => continue,
            };
            if response.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(error) = response.get("error") {
                return Err(anyhow!("Codex app-server error: {}", error));
            }
            return Ok(response.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    fn notify(&mut self, method: &str, params: Value) -> Result<()> {
        serde_json::to_writer(
            &mut self.stdin,
            &json!({"jsonrpc": "2.0", "method": method, "params": params}),
        )?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;
        Ok(())
    }

    fn close(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn process_codex_event(
    event: &Value,
    request_id: u64,
    response_seen: &mut bool,
    output: &mut String,
) -> Result<Option<TurnResult>> {
    if event.get("id").and_then(Value::as_u64) == Some(request_id) {
        *response_seen = true;
        if let Some(error) = event.get("error") {
            return Err(anyhow!("Codex turn failed: {}", error));
        }
        return Ok(None);
    }

    match event.get("method").and_then(Value::as_str).unwrap_or("") {
        "item/agentMessage/delta" => {
            if let Some(delta) = event
                .get("params")
                .and_then(|params| params.get("delta"))
                .and_then(Value::as_str)
            {
                output.push_str(delta);
            }
        }
        "turn/completed" if *response_seen || event.get("params").is_some() => {
            let status = nested_string(event, &["params", "turn", "status"])
                .or_else(|| nested_string(event, &["params", "status"]))
                .unwrap_or_else(|| "completed".to_string());
            if status != "completed" {
                return Err(anyhow!("Codex turn ended with status {}", status));
            }
            return Ok(Some(TurnResult {
                output: std::mem::take(output),
                tokens_used: None,
            }));
        }
        "turn/failed" | "turn/interrupted" => {
            return Err(anyhow!("Codex turn did not complete"));
        }
        _ => {}
    }
    Ok(None)
}

fn write_jsonrpc<W: Write>(writer: &mut W, id: u64, method: &str, params: Value) -> Result<()> {
    serde_json::to_writer(
        &mut *writer,
        &json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}),
    )?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn nested_string(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str().map(str::to_string)
}

fn usage_tokens(value: Option<&Value>) -> Option<i64> {
    let object = value?.as_object()?;
    let input = object
        .get("input_tokens")
        .or_else(|| object.get("inputTokens"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let output = object
        .get("output_tokens")
        .or_else(|| object.get("outputTokens"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    (input + output > 0).then_some(input + output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn claude_parser_accumulates_streamed_text_and_usage() {
        let mut output = String::new();
        assert!(process_claude_event(
            &json!({"type":"assistant","message":{"content":[{"type":"text","text":"hello "}]}}),
            &mut output,
        )
        .unwrap()
        .is_none());
        assert!(process_claude_event(
            &json!({"type":"content_block_delta","delta":{"text":"world"}}),
            &mut output,
        )
        .unwrap()
        .is_none());
        let result = process_claude_event(
            &json!({"type":"result","usage":{"input_tokens":3,"output_tokens":5}}),
            &mut output,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            result,
            TurnResult {
                output: "hello world".into(),
                tokens_used: Some(8)
            }
        );
    }

    #[test]
    fn claude_parser_reports_provider_errors() {
        let error = process_claude_event(
            &json!({"type":"result","is_error":true,"result":"rate limited"}),
            &mut String::new(),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("rate limited"));
    }

    #[test]
    fn codex_parser_waits_for_completion_and_preserves_deltas() {
        let mut output = String::new();
        let mut response_seen = false;
        assert!(process_codex_event(
            &json!({"id":7,"result":{}}),
            7,
            &mut response_seen,
            &mut output,
        )
        .unwrap()
        .is_none());
        assert!(process_codex_event(
            &json!({"method":"item/agentMessage/delta","params":{"delta":"done"}}),
            7,
            &mut response_seen,
            &mut output,
        )
        .unwrap()
        .is_none());
        let result = process_codex_event(
            &json!({"method":"turn/completed","params":{"turn":{"status":"completed"}}}),
            7,
            &mut response_seen,
            &mut output,
        )
        .unwrap()
        .unwrap();
        assert_eq!(result.output, "done");
    }

    #[test]
    fn codex_parser_rejects_failed_and_interrupted_turns() {
        for method in ["turn/failed", "turn/interrupted"] {
            let error = process_codex_event(
                &json!({"method":method}),
                1,
                &mut false,
                &mut String::new(),
            )
            .unwrap_err()
            .to_string();
            assert!(error.contains("did not complete"));
        }
    }

    #[test]
    fn usage_parser_supports_camel_case_fields() {
        assert_eq!(
            usage_tokens(Some(&json!({"inputTokens": 4, "outputTokens": 6}))),
            Some(10)
        );
        assert_eq!(
            usage_tokens(Some(&json!({"input_tokens": 0, "output_tokens": 0}))),
            None
        );
    }

    struct FakeTransport {
        prompts: Vec<String>,
        fail: bool,
    }

    impl TurnTransport for FakeTransport {
        fn send_turn(&mut self, prompt: &str) -> Result<TurnResult> {
            self.prompts.push(prompt.to_string());
            if self.fail {
                Err(anyhow!("simulated provider failure"))
            } else {
                Ok(TurnResult {
                    output: "acknowledged".to_string(),
                    tokens_used: Some(2),
                })
            }
        }

        fn close(&mut self) {}
    }

    fn bridge_test_database() -> Database {
        let db = Database::in_memory_for_tests().unwrap();
        db.register_agent("sender", "sender", "codex", "default", None, &[])
            .unwrap();
        db.register_agent("receiver", "receiver", "claude", "default", None, &[])
            .unwrap();
        db.send_bridge_message(
            "msg_live", "sender", "receiver", None, "request", "do the work", None,
        )
        .unwrap();
        db
    }

    #[test]
    fn turn_loop_delivers_bridge_context_at_boundary_and_acks_after_success() {
        let db = bridge_test_database();
        let (sender, input) = mpsc::channel();
        sender.send(None).unwrap();
        let mut transport = FakeTransport {
            prompts: Vec::new(),
            fail: false,
        };

        run_turn_loop(&db, "receiver", "claude", &mut transport, input).unwrap();

        assert_eq!(transport.prompts.len(), 1);
        assert!(transport.prompts[0].contains("do the work"));
        assert!(db.get_bridge_inbox("receiver", false).unwrap().is_empty());
        assert_eq!(db.get_usage("claude").unwrap(), 2);
    }

    #[test]
    fn failed_turn_leaves_claimed_message_unacknowledged_for_retry() {
        let db = bridge_test_database();
        let (sender, input) = mpsc::channel();
        sender.send(None).unwrap();
        let mut transport = FakeTransport {
            prompts: Vec::new(),
            fail: true,
        };

        assert!(run_turn_loop(&db, "receiver", "claude", &mut transport, input).is_err());
        let messages = db.get_bridge_inbox("receiver", true).unwrap();
        assert_eq!(messages[0].state, "claimed");
        assert!(messages[0].acked_at.is_none());
    }
}
