use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_DIR_SEQ: AtomicU64 = AtomicU64::new(0);

// Every test run gets its own data directory so parallel tests never share
// state and the real user data directory is never touched.
fn test_data_dir() -> PathBuf {
    let seq = TEST_DIR_SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("deepmate-cli-test-{}-{seq}", std::process::id()))
}

fn deepmate_in(dir: &PathBuf, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_deepmate"))
        .args(args)
        .env("DEEPMATE_DATA_DIR", dir)
        .output()
        .expect("failed to run deepmate binary")
}

fn deepmate(args: &[&str]) -> std::process::Output {
    deepmate_in(&test_data_dir(), args)
}

fn deepmate_ok(args: &[&str]) -> String {
    let output = deepmate(args);
    assert!(
        output.status.success(),
        "`deepmate {}` failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn adapters_command_lists_test_adapter() {
    let stdout = deepmate_ok(&["--adapter", "test", "adapters"]);
    assert!(stdout.contains("test"));
}

#[test]
fn detect_command_reports_fake_harness() {
    let stdout = deepmate_ok(&["--adapter", "test", "detect"]);
    assert!(stdout.contains("found: true"));
    assert!(stdout.contains("harness: test"));
}

#[test]
fn detect_command_supports_json_output() {
    let stdout = deepmate_ok(&["--adapter", "test", "detect", "--json"]);
    assert!(stdout.contains("\"found\""));
    assert!(stdout.contains("9.9.9-test"));
}

#[test]
fn status_command_supports_json_output() {
    let stdout = deepmate_ok(&["--adapter", "test", "status", "--json"]);
    assert!(stdout.contains("\"kind\""));
    assert!(stdout.contains("installed"));
}

#[test]
fn doctor_command_returns_fake_check() {
    let stdout = deepmate_ok(&["--adapter", "test", "doctor", "--json"]);
    assert!(stdout.contains("fake.healthy"));
}

#[test]
fn profile_list_returns_default_profile() {
    let stdout = deepmate_ok(&["--adapter", "test", "profile", "list"]);
    assert!(stdout.contains("default"));
}

#[test]
fn provider_list_returns_demo_provider() {
    let stdout = deepmate_ok(&["--adapter", "test", "provider", "list", "--json"]);
    assert!(stdout.contains("demo"));
    assert!(stdout.contains("openai-compatible"));
}

#[test]
fn model_list_returns_demo_chat() {
    let stdout = deepmate_ok(&["--adapter", "test", "model", "list"]);
    assert!(stdout.contains("demo-chat"));
}

#[test]
fn plugin_list_returns_fake_plugin() {
    let stdout = deepmate_ok(&["--adapter", "test", "plugin", "list"]);
    assert!(stdout.contains("fake-plugin"));
    assert!(stdout.contains("enabled"));
}

#[test]
fn unknown_adapter_is_an_error() {
    let output = deepmate(&["--adapter", "nope", "status"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown adapter"));
}

#[test]
fn runtime_commands_accept_fake_adapter() {
    for action in ["start", "stop", "restart"] {
        let stdout = deepmate_ok(&["--adapter", "test", "runtime", action, "--json"]);
        assert!(stdout.contains("\"ok\":true"), "runtime {action} failed");
    }
}

#[test]
fn capability_gate_blocks_unsupported_commands() {
    // The `minimal` fake adapter only supports runtime; profile management
    // must be rejected instead of silently returning empty lists.
    for (command, what) in [
        ("profile", "profiles"),
        ("provider", "providers"),
        ("model", "models"),
        ("plugin", "plugins"),
    ] {
        let output = deepmate(&["--adapter", "minimal", command, "list"]);
        assert!(!output.status.success(), "{command} list should be gated");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(&format!("does not support {what}")),
            "stderr: {stderr}"
        );
    }
}

fn write_profile_fixture(home: &Path, name: &str, deps: &[(&str, &str)]) {
    let dir = home.join("profiles").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    let mut deps_map = serde_json::Map::new();
    for (dep, version) in deps {
        deps_map.insert(
            dep.to_string(),
            serde_json::Value::String(version.to_string()),
        );
    }
    let manifest = serde_json::json!({
        "name": format!("dsh-profile-{name}"),
        "private": true,
        "dependencies": deps_map,
        "dsh": { "profile": { "bundles": ["@deepseek-ai/dsh-base"] } }
    });
    std::fs::write(
        dir.join("package.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
}

#[test]
fn profile_list_reads_dsh_home() {
    let dir = test_data_dir();
    let home = test_data_dir();
    write_profile_fixture(&home, "web", &[]);
    write_profile_fixture(&home, "headless", &[]);
    let output = Command::new(env!("CARGO_BIN_EXE_deepmate"))
        .args(["--adapter", "deepseek-harness", "profile", "list"])
        .env("DEEPMATE_DATA_DIR", &dir)
        .env("DSH_HOME", &home)
        .output()
        .expect("failed to run deepmate binary");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("headless"));
    assert!(stdout.contains("web"));
}

#[test]
fn plugin_list_reads_dsh_home() {
    let dir = test_data_dir();
    let home = test_data_dir();
    write_profile_fixture(&home, "web", &[("turtle-ui", "^1.0.0")]);
    let output = Command::new(env!("CARGO_BIN_EXE_deepmate"))
        .args(["--adapter", "deepseek-harness", "plugin", "list"])
        .env("DEEPMATE_DATA_DIR", &dir)
        .env("DSH_HOME", &home)
        .output()
        .expect("failed to run deepmate binary");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("web/turtle-ui"));
}

#[test]
fn provider_and_model_list_read_settings() {
    let dir = test_data_dir();
    let home = test_data_dir();
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        home.join("settings.yaml"),
        "llm-pi-ai:\n  providers:\n    openai:\n      displayName: OpenAI\n      models:\n        - id: gpt-4o\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_deepmate"))
        .args(["--adapter", "deepseek-harness", "provider", "list"])
        .env("DEEPMATE_DATA_DIR", &dir)
        .env("DSH_HOME", &home)
        .output()
        .expect("failed to run deepmate binary");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("deepseek-official"));
    assert!(stdout.contains("openai — OpenAI (pi-ai)"));

    let output = Command::new(env!("CARGO_BIN_EXE_deepmate"))
        .args(["--adapter", "deepseek-harness", "model", "list"])
        .env("DEEPMATE_DATA_DIR", &dir)
        .env("DSH_HOME", &home)
        .output()
        .expect("failed to run deepmate binary");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("deepseek-v4-flash"));
    assert!(stdout.contains("gpt-4o"));
}

#[test]
fn history_records_actions() {
    let dir = test_data_dir();
    let output = deepmate_in(&dir, &["--adapter", "test", "status"]);
    assert!(output.status.success());
    let history = dir.join("history").join("actions.jsonl");
    let text = std::fs::read_to_string(&history).expect("history file should exist");
    assert!(text.contains("\"action\":\"cli.status\""));
    assert!(text.contains("\"adapter\":\"test\""));
}

#[test]
fn data_dir_flag_overrides_default() {
    let dir = test_data_dir();
    let output = Command::new(env!("CARGO_BIN_EXE_deepmate"))
        .args([
            "--data-dir",
            dir.to_str().unwrap(),
            "--adapter",
            "test",
            "status",
        ])
        .output()
        .expect("failed to run deepmate binary");
    assert!(output.status.success());
    assert!(dir.join("history").join("actions.jsonl").exists());
}

#[test]
fn default_config_is_written_on_first_run() {
    let dir = test_data_dir();
    let output = deepmate_in(&dir, &["--adapter", "test", "status"]);
    assert!(output.status.success());
    let config =
        std::fs::read_to_string(dir.join("config.toml")).expect("config should be written");
    assert!(config.contains("auto_start"));
}
