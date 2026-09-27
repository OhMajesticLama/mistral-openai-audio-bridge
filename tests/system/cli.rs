// Story: 005 — CLI system tests: the real binary, each argument observed
// end-to-end (--help/--version/unknown-flag exit behavior, and every
// config flag honored by the running process).

use std::io::Read;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::connect_async;

const SSE_DONE: &str = concat!(
    "data: {\"type\":\"transcript.text.done\",\"text\":\" Bonjour\"}\n\n",
    "\n",
);

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_vibe-audio-bridge"))
}

#[test]
fn test_help_prints_usage_and_exits_zero() {
    for flag in ["--help", "-h"] {
        let out = bin().arg(flag).output().unwrap();
        assert!(out.status.success(), "{flag} exited with {out:?}");
        let stdout = String::from_utf8_lossy(&out.stdout);
        for opt in ["--listen", "--upstream", "--log-level", "--debug-dump", "--dump-dir"] {
            assert!(stdout.contains(opt), "usage missing {opt}:\n{stdout}");
        }
        // Story: 009 — shorthands are documented in the usage.
        for short in ["-l", "-u", "-d", "-D"] {
            assert!(stdout.contains(short), "usage missing shorthand {short}:\n{stdout}");
        }
    }
}

#[test]
fn test_version_prints_version_and_exits_zero() {
    for flag in ["--version", "-V"] {
        let out = bin().arg(flag).output().unwrap();
        assert!(out.status.success(), "{flag} exited with {out:?}");
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains(env!("CARGO_PKG_VERSION")), "no version in {stdout:?}");
    }
}

#[test]
fn test_unknown_flag_fails_without_serving() {
    let out = bin().arg("--no-such-flag").output().unwrap();
    assert!(!out.status.success(), "unknown flag exited successfully");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("--no-such-flag"), "error should name the flag:\n{stderr}");
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

async fn spawn_mock_upstream() -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = axum::Router::new().route(
        "/v1/audio/transcriptions",
        axum::routing::post(|| async {
            (
                axum::http::StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                SSE_DONE.to_string(),
            )
        }),
    );
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

fn wait_listening(port: u16) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(
            Instant::now() < deadline,
            "bridge did not start listening on {port}"
        );
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn spawn_bridge(args: &[&str]) -> (Child, u16) {
    let port = free_port();
    let child = Command::new(env!("CARGO_BIN_EXE_vibe-audio-bridge"))
        .args(args)
        .args(["--listen", &format!("127.0.0.1:{port}")])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    wait_listening(port);
    (child, port)
}

fn read_output(child: &mut Child) -> String {
    let mut out = String::new();
    if let Some(mut stdout) = child.stdout.take() {
        stdout.read_to_string(&mut out).unwrap();
    }
    if let Some(mut stderr) = child.stderr.take() {
        stderr.read_to_string(&mut out).unwrap();
    }
    out
}

async fn transcribe_once(port: u16) -> String {
    let url = format!("ws://127.0.0.1:{port}/v1/audio/transcriptions/realtime");
    let (mut ws, _) = connect_async(url.into_client_request().unwrap()).await.unwrap();
    let ev: Value = serde_json::from_str(&ws.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
    assert_eq!(ev["type"], "session.created");
    let pcm = vec![7u8; 64];
    ws.send(Message::Text(
        serde_json::json!({"type": "input_audio.append", "audio": STANDARD.encode(&pcm)}).to_string().into(),
    ))
    .await
    .unwrap();
    ws.send(Message::Text(serde_json::json!({"type": "input_audio.end"}).to_string().into()))
    .await
    .unwrap();
    loop {
        let ev: Value =
            serde_json::from_str(&ws.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
        match ev["type"].as_str().unwrap_or_default() {
            "transcription.done" => return ev["text"].as_str().unwrap().to_string(),
            "error" => panic!("bridge error: {ev}"),
            _ => {}
        }
    }
}

#[tokio::test]
async fn test_system_listen_and_upstream_flags() {
    let upstream = spawn_mock_upstream().await;
    let (mut child, port) = spawn_bridge(&["--upstream", &format!("http://{upstream}")]);
    let text = transcribe_once(port).await;
    child.kill().unwrap();
    assert_eq!(text, " Bonjour", "--listen/--upstream not honored end-to-end");
}

#[tokio::test]
async fn test_system_debug_dump_flags() {
    let upstream = spawn_mock_upstream().await;
    let dump_dir = std::env::temp_dir().join(format!("cli-sys-dump-{}", std::process::id()));
    std::fs::remove_dir_all(&dump_dir).ok();
    let (mut child, port) = spawn_bridge(&[
        "--upstream",
        &format!("http://{upstream}"),
        "--debug-dump",
        "--dump-dir",
        &dump_dir.to_string_lossy(),
    ]);
    let _ = transcribe_once(port).await;
    std::thread::sleep(Duration::from_millis(200));
    child.kill().unwrap();
    let files: Vec<_> = std::fs::read_dir(&dump_dir).unwrap().collect();
    assert_eq!(files.len(), 1, "expected one dump file");
    let contents = std::fs::read(files[0].as_ref().unwrap().path()).unwrap();
    assert_eq!(contents, vec![7u8; 64], "dump contents != sent PCM");
    std::fs::remove_dir_all(&dump_dir).ok();
}

#[tokio::test]
async fn test_system_log_level_flag() {
    let upstream = spawn_mock_upstream().await;
    let (mut child, port) = spawn_bridge(&[
        "--upstream",
        &format!("http://{upstream}"),
        "--log-level",
        "error",
    ]);
    let _ = transcribe_once(port).await;
    child.kill().unwrap();
    let out = read_output(&mut child);
    assert!(
        !out.contains("listening on") && !out.contains("INFO"),
        "--log-level error still logged info:\n{out}"
    );
}

#[tokio::test]
async fn test_system_env_configures_and_cli_overrides() {
    // Story: 005 — environment variables configure the bridge, and CLI flags
    // win over them (VIBE_BRIDGE_LISTEN must lose to --listen).
    let upstream = spawn_mock_upstream().await;
    let port = free_port();
    let mut child = Command::new(env!("CARGO_BIN_EXE_vibe-audio-bridge"))
        .env_clear()
        .env("VIBE_BRIDGE_LISTEN", "127.0.0.1:1")
        .env("VIBE_BRIDGE_UPSTREAM", format!("http://{upstream}"))
        .args(["--listen", &format!("127.0.0.1:{port}")])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    wait_listening(port);
    let text = transcribe_once(port).await;
    child.kill().unwrap();
    assert_eq!(text, " Bonjour", "env upstream not honored or env listen not overridden");
}

#[tokio::test]
async fn test_system_debug_dump_env_enables_dumps() {
    // VIBE_BRIDGE_DEBUG_DUMP=1 enables dumps, with VIBE_BRIDGE_DUMP_DIR selecting
    // the directory.
    let upstream = spawn_mock_upstream().await;
    let dump_dir = std::env::temp_dir().join(format!("cli-sys-dump-switch-{}", std::process::id()));
    std::fs::remove_dir_all(&dump_dir).ok();
    let port = free_port();
    let mut child = Command::new(env!("CARGO_BIN_EXE_vibe-audio-bridge"))
        .env_clear()
        .env("VIBE_BRIDGE_UPSTREAM", format!("http://{upstream}"))
        .env("VIBE_BRIDGE_DEBUG_DUMP", "1")
        .env("VIBE_BRIDGE_DUMP_DIR", &dump_dir)
        .args(["--listen", &format!("127.0.0.1:{port}")])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    wait_listening(port);
    let _ = transcribe_once(port).await;
    std::thread::sleep(Duration::from_millis(200));
    child.kill().unwrap();
    let files: Vec<_> = std::fs::read_dir(&dump_dir).unwrap().collect();
    assert_eq!(files.len(), 1, "expected one dump file");
    let contents = std::fs::read(files[0].as_ref().unwrap().path()).unwrap();
    assert_eq!(contents, vec![7u8; 64], "dump contents != sent PCM");
    std::fs::remove_dir_all(&dump_dir).ok();
}

#[tokio::test]
async fn test_system_dump_dir_env_alone_enables_dumps() {
    // Setting a dump directory — flag or env — enables dumps; the env var no
    // longer requires VIBE_BRIDGE_DEBUG_DUMP=1 as well.
    let upstream = spawn_mock_upstream().await;
    let dump_dir = std::env::temp_dir().join(format!("cli-sys-dump-env-{}", std::process::id()));
    std::fs::remove_dir_all(&dump_dir).ok();
    let port = free_port();
    let mut child = Command::new(env!("CARGO_BIN_EXE_vibe-audio-bridge"))
        .env_clear()
        .env("VIBE_BRIDGE_UPSTREAM", format!("http://{upstream}"))
        .env("VIBE_BRIDGE_DUMP_DIR", &dump_dir)
        .args(["--listen", &format!("127.0.0.1:{port}")])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    wait_listening(port);
    let _ = transcribe_once(port).await;
    std::thread::sleep(Duration::from_millis(200));
    child.kill().unwrap();
    let files: Vec<_> = std::fs::read_dir(&dump_dir).unwrap().collect();
    assert_eq!(files.len(), 1, "expected one dump file");
    let contents = std::fs::read(files[0].as_ref().unwrap().path()).unwrap();
    assert_eq!(contents, vec![7u8; 64], "dump contents != sent PCM");
    std::fs::remove_dir_all(&dump_dir).ok();
}
