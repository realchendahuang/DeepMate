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
fn market_list_returns_known_sources() {
    let stdout = deepmate_ok(&["--adapter", "test", "market", "list"]);
    assert!(stdout.contains("curated"));
    assert!(stdout.contains("community"));
}

#[test]
fn market_search_returns_fake_entry() {
    let stdout = deepmate_ok(&["--adapter", "test", "market", "search", "fake-market"]);
    assert!(stdout.contains("fake-market-plugin"));
    assert!(stdout.contains("fake-publisher"));
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

// Runs a deepseek-harness command against a caller-owned DSH_HOME and returns
// (stdout, stderr), so write-then-read steps within one test share the same
// harness state.
fn dsh_in(home: &Path, args: &[&str]) -> (String, String) {
    let dir = test_data_dir();
    std::fs::create_dir_all(home).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_deepmate"))
        .args(["--adapter", "deepseek-harness"])
        .args(args)
        .env("DEEPMATE_DATA_DIR", &dir)
        .env("DSH_HOME", home)
        .output()
        .expect("failed to run deepmate binary");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn provider_set_writes_settings_and_reads_back() {
    let home = test_data_dir();
    let (_, stderr) = dsh_in(
        &home,
        &[
            "provider",
            "set",
            "ollama",
            "Ollama",
            "--api",
            "openai-completions",
            "--base-url",
            "http://localhost:11434/v1",
            "--api-key-env",
            "OLLAMA_KEY",
        ],
    );
    assert!(stderr.is_empty(), "stderr: {stderr}");

    let (stdout, _) = dsh_in(&home, &["provider", "list"]);
    assert!(
        stdout.contains("ollama — Ollama (pi-ai)"),
        "stdout: {stdout}"
    );
}

#[test]
fn provider_remove_deletes_route() {
    let home = test_data_dir();
    let (_, stderr) = dsh_in(
        &home,
        &["provider", "set", "temp", "Temp", "--api-key-env", "TMP"],
    );
    assert!(stderr.is_empty(), "stderr: {stderr}");
    let (_, stderr) = dsh_in(&home, &["provider", "remove", "temp"]);
    assert!(stderr.is_empty(), "stderr: {stderr}");
    let (stdout, _) = dsh_in(&home, &["provider", "list"]);
    assert!(
        !stdout.contains("temp"),
        "provider should be removed: {stdout}"
    );
}

#[test]
fn model_set_and_remove_roundtrip() {
    let home = test_data_dir();
    let (_, stderr) = dsh_in(
        &home,
        &["provider", "set", "demo", "Demo", "--api-key-env", "DKEY"],
    );
    assert!(stderr.is_empty(), "stderr: {stderr}");
    let (_, stderr) = dsh_in(
        &home,
        &[
            "model",
            "set",
            "--provider",
            "demo",
            "my-model",
            "--name",
            "My Model",
            "--context-window",
            "8192",
            "--max-tokens",
            "4096",
            "--input",
            "text,image",
        ],
    );
    assert!(stderr.is_empty(), "stderr: {stderr}");

    let (stdout, _) = dsh_in(&home, &["model", "list"]);
    assert!(stdout.contains("my-model"), "stdout: {stdout}");

    let (_, stderr) = dsh_in(
        &home,
        &["model", "remove", "--provider", "demo", "my-model"],
    );
    assert!(stderr.is_empty(), "stderr: {stderr}");
    let (stdout, _) = dsh_in(&home, &["model", "list"]);
    assert!(
        !stdout.contains("my-model"),
        "model should be removed: {stdout}"
    );
}

#[test]
fn deepseek_official_route_edits_llm_deepseek() {
    let home = test_data_dir();
    let (_, stderr) = dsh_in(
        &home,
        &[
            "provider",
            "set",
            "deepseek-official",
            "DeepSeek",
            "--base-url",
            "https://api.deepseek.com",
        ],
    );
    assert!(stderr.is_empty(), "stderr: {stderr}");

    let (stdout, _) = dsh_in(&home, &["provider", "list"]);
    assert!(stdout.contains("deepseek-official"));

    // The built-in route cannot be removed.
    let (_, stderr) = dsh_in(&home, &["provider", "remove", "deepseek-official"]);
    assert!(
        !stderr.is_empty(),
        "deepseek-official removal must be rejected"
    );
}

#[test]
fn profile_create_and_remove_via_cli() {
    let home = test_data_dir();
    let (_, stderr) = dsh_in(&home, &["profile", "create", "tui"]);
    assert!(stderr.is_empty(), "stderr: {stderr}");
    let (stdout, _) = dsh_in(&home, &["profile", "list"]);
    assert!(stdout.contains("tui"), "stdout: {stdout}");

    let (_, stderr) = dsh_in(&home, &["profile", "remove", "tui"]);
    assert!(stderr.is_empty(), "stderr: {stderr}");
    let (stdout, _) = dsh_in(&home, &["profile", "list"]);
    assert!(
        !stdout.contains("tui"),
        "profile should be removed: {stdout}"
    );
}

#[test]
fn config_edit_commands_are_capability_gated() {
    // The `minimal` fake adapter only supports runtime; every config edit
    // must be rejected rather than silently succeeding.
    for (command, what) in [
        (vec!["profile", "create", "x"], "profiles"),
        (vec!["provider", "set", "x", "X"], "providers"),
        (vec!["provider", "remove", "x"], "providers"),
        (vec!["model", "set", "--provider", "p", "m"], "models"),
        (vec!["model", "remove", "--provider", "p", "m"], "models"),
    ] {
        let output = deepmate_with_minimal(&command);
        assert!(!output.status.success(), "{command:?} should be gated");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(&format!("does not support {what}")),
            "stderr: {stderr}"
        );
    }
}

fn deepmate_with_minimal(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_deepmate"))
        .args(["--adapter", "minimal"])
        .args(args)
        .output()
        .expect("failed to run deepmate binary")
}

// Runs a pi-agent command against an isolated PI_HOME.
fn pi_in(home: &Path, args: &[&str]) -> std::process::Output {
    let dir = test_data_dir();
    std::fs::create_dir_all(home).unwrap();
    Command::new(env!("CARGO_BIN_EXE_deepmate"))
        .args(["--adapter", "pi-agent"])
        .args(args)
        .env("DEEPMATE_DATA_DIR", &dir)
        .env("PI_HOME", home)
        .output()
        .expect("failed to run deepmate binary")
}

fn write_pi_models(home: &Path) {
    let agent = home.join("agent");
    std::fs::create_dir_all(&agent).unwrap();
    std::fs::write(
        agent.join("models.json"),
        r#"{
  "providers": {
    "ds": {
      "name": "DeepSeek Official",
      "baseUrl": "https://api.deepseek.com",
      "api": "openai-responses",
      "apiKey": "sk-secret",
      "models": [
        { "id": "deepseek-v4-flash", "name": "DeepSeek V4 Flash", "contextWindow": 1000000, "maxTokens": 131072 }
      ]
    }
  }
}"#,
    )
    .unwrap();
}

fn write_pi_settings(home: &Path) {
    let agent = home.join("agent");
    std::fs::create_dir_all(&agent).unwrap();
    std::fs::write(
        agent.join("settings.json"),
        r#"{"packages": ["npm:pi-subagents"]}"#,
    )
    .unwrap();
}

#[test]
fn pi_agent_lists_providers_models_plugins() {
    let home = test_data_dir();
    write_pi_models(&home);
    write_pi_settings(&home);

    let providers = pi_in(&home, &["provider", "list"]);
    assert!(providers.status.success());
    let stdout = String::from_utf8_lossy(&providers.stdout);
    assert!(
        stdout.contains("ds — DeepSeek Official (pi)"),
        "stdout: {stdout}"
    );
    // The plaintext apiKey must never leak into output.
    assert!(!stdout.contains("sk-secret"), "secret leaked: {stdout}");

    let models = pi_in(&home, &["model", "list"]);
    assert!(models.status.success());
    assert!(String::from_utf8_lossy(&models.stdout).contains("deepseek-v4-flash"));

    let plugins = pi_in(&home, &["plugin", "list"]);
    assert!(plugins.status.success());
    assert!(String::from_utf8_lossy(&plugins.stdout).contains("npm:pi-subagents"));
}

#[test]
fn pi_agent_capability_gate_rejects_runtime_and_profiles() {
    let home = test_data_dir();
    write_pi_models(&home);

    for (args, what) in [
        (vec!["runtime", "start"], "runtime control"),
        (vec!["profile", "list"], "profiles"),
    ] {
        let output = pi_in(&home, &args);
        assert!(!output.status.success(), "{args:?} should be gated");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(&format!("does not support {what}")),
            "stderr: {stderr}"
        );
    }
}

#[test]
fn snapshot_export_list_import_roundtrip() {
    let dir = test_data_dir();
    let export = deepmate_in(&dir, &["--adapter", "test", "snapshot", "export", "coding"]);
    assert!(
        export.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&export.stderr)
    );
    assert!(dir.join("snapshots").join("coding.json").exists());

    let list = deepmate_in(&dir, &["--adapter", "test", "snapshot", "list"]);
    assert!(list.status.success());
    assert!(String::from_utf8_lossy(&list.stdout).contains("coding"));

    let import = deepmate_in(&dir, &["--adapter", "test", "snapshot", "import", "coding"]);
    assert!(
        import.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&import.stderr)
    );
    assert!(String::from_utf8_lossy(&import.stdout).contains("imported snapshot coding"));
}

#[test]
fn snapshot_import_rejects_mismatched_adapter() {
    let dir = test_data_dir();
    // Export from the fake adapter, then try to import into pi-agent using
    // the same data dir (so the snapshot is found) but a different adapter.
    let export = deepmate_in(&dir, &["--adapter", "test", "snapshot", "export", "coding"]);
    assert!(export.status.success());

    let home = test_data_dir();
    write_pi_models(&home);
    let import = Command::new(env!("CARGO_BIN_EXE_deepmate"))
        .args(["--adapter", "pi-agent", "snapshot", "import", "coding"])
        .env("DEEPMATE_DATA_DIR", &dir)
        .env("PI_HOME", &home)
        .output()
        .expect("failed to run deepmate binary");
    assert!(!import.status.success());
    let stderr = String::from_utf8_lossy(&import.stderr);
    assert!(stderr.contains("adapter 'test'"), "stderr: {stderr}");
}
