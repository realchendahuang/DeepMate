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

fn deepmate_ok(args: &[&str]) -> String {
    let output = deepmate_in(&test_data_dir(), args);
    assert!(
        output.status.success(),
        "`deepmate {}` failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn detect_command_supports_json_output() {
    // Detection depends on whether the machine has the harness CLI, so the
    // test only asserts the JSON envelope shape.
    let stdout = deepmate_ok(&["detect", "--json"]);
    assert!(stdout.contains("\"found\""));
}

#[test]
fn status_command_supports_json_output() {
    let stdout = deepmate_ok(&["status", "--json"]);
    assert!(stdout.contains("\"kind\""));
}

#[test]
fn doctor_command_reports_checks() {
    // Doctor always succeeds; the runtime check may pass or fail depending on
    // whether the harness CLI is installed on the test machine.
    let stdout = deepmate_ok(&["doctor", "--json"]);
    assert!(stdout.contains("runtime.installed"));
}

#[test]
fn market_list_returns_known_sources() {
    // market_sources is static information; no network is involved.
    let stdout = deepmate_ok(&["market", "list"]);
    assert!(stdout.contains("curated"));
    assert!(stdout.contains("community"));
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
        .args(["profile", "list"])
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
        .args(["plugin", "list"])
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
        .args(["provider", "list"])
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
        .args(["model", "list"])
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
    let output = deepmate_in(&dir, &["status"]);
    assert!(output.status.success());
    let history = dir.join("history").join("actions.jsonl");
    let text = std::fs::read_to_string(&history).expect("history file should exist");
    assert!(text.contains("\"action\":\"cli.status\""));
    assert!(!text.contains("adapter"));
}

#[test]
fn data_dir_flag_overrides_default() {
    let dir = test_data_dir();
    let output = Command::new(env!("CARGO_BIN_EXE_deepmate"))
        .args(["--data-dir", dir.to_str().unwrap(), "status"])
        .output()
        .expect("failed to run deepmate binary");
    assert!(output.status.success());
    assert!(dir.join("history").join("actions.jsonl").exists());
}

#[test]
fn default_config_is_written_on_first_run() {
    let dir = test_data_dir();
    let output = deepmate_in(&dir, &["status"]);
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

// A fake `dsh` that keeps the test hermetic (no npm traffic) while recording
// the exact arguments the bootstrap forwards. The launcher is a POSIX shell
// script, so the test is unix-only: Windows would need a batch-file dialect
// that cannot be verified on the project's macOS build host, and the project
// ships macOS artifacts only (see AGENTS.md). The forwarded-argument logic
// itself is platform-independent Rust and is covered by this test on macOS
// and Linux.
#[cfg(unix)]
#[test]
fn profile_create_and_remove_via_cli() {
    let home = test_data_dir();
    let work = test_data_dir();
    std::fs::create_dir_all(&work).unwrap();
    let recorded = work.join("recorded-args");
    let fake = {
        use std::os::unix::fs::PermissionsExt;
        let fake = work.join("fake-dsh");
        std::fs::write(
            &fake,
            format!(
                "#!/bin/sh\n[ \"$1\" = \"--version\" ] && exit 0\nprintf '%s\\n' \"$@\" > \"{}\"\nexit 0\n",
                recorded.display()
            ),
        )
        .unwrap();
        let mut perms = std::fs::metadata(&fake).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&fake, perms).unwrap();
        fake
    };

    let run = |args: &[&str]| {
        let dir = test_data_dir();
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_deepmate"))
            .args(args)
            .env("DEEPMATE_DATA_DIR", &dir)
            .env("DSH_HOME", &home)
            .env("DEEPMATE_DSH_BIN", &fake)
            .output()
            .expect("failed to run deepmate binary");
        (
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    };

    let (_, stderr) = run(&["profile", "create", "tui"]);
    // DeepMate's own INFO tracing may land on stderr; only errors matter.
    assert!(!stderr.to_lowercase().contains("error"), "stderr: {stderr}");
    // The bootstrap forwarded the base + web-app bundles to the profile.
    let recorded_args = std::fs::read_to_string(&recorded).unwrap();
    let args: Vec<&str> = recorded_args.lines().collect();
    assert_eq!(
        args,
        [
            "plugin",
            "--profile",
            "tui",
            "add",
            "@deepseek-ai/dsh-base",
            "@deepseek-ai/dsh-web-app"
        ]
    );
    // The scenario got its own settings document redirection.
    assert!(home.join("profiles/tui/cordis.patch.yml").is_file());

    let (stdout, _) = run(&["profile", "list"]);
    assert!(stdout.contains("tui"), "stdout: {stdout}");

    let (_, stderr) = run(&["profile", "remove", "tui"]);
    assert!(stderr.is_empty(), "stderr: {stderr}");
    let (stdout, _) = run(&["profile", "list"]);
    assert!(
        !stdout.contains("tui"),
        "profile should be removed: {stdout}"
    );
}

#[test]
fn snapshot_export_list_import_roundtrip() {
    let dir = test_data_dir();
    // An isolated DSH_HOME keeps the capture hermetic; the built-in
    // deepseek-official route still appears, so the roundtrip carries data.
    let home = test_data_dir();
    std::fs::create_dir_all(&home).unwrap();

    let export = Command::new(env!("CARGO_BIN_EXE_deepmate"))
        .args(["snapshot", "export", "coding"])
        .env("DEEPMATE_DATA_DIR", &dir)
        .env("DSH_HOME", &home)
        .output()
        .expect("failed to run deepmate binary");
    assert!(
        export.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&export.stderr)
    );
    assert!(dir.join("snapshots").join("coding.json").exists());

    let list = Command::new(env!("CARGO_BIN_EXE_deepmate"))
        .args(["snapshot", "list"])
        .env("DEEPMATE_DATA_DIR", &dir)
        .output()
        .expect("failed to run deepmate binary");
    assert!(list.status.success());
    assert!(String::from_utf8_lossy(&list.stdout).contains("coding"));

    let import = Command::new(env!("CARGO_BIN_EXE_deepmate"))
        .args(["snapshot", "import", "coding"])
        .env("DEEPMATE_DATA_DIR", &dir)
        .env("DSH_HOME", &home)
        .output()
        .expect("failed to run deepmate binary");
    assert!(
        import.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&import.stderr)
    );
    assert!(String::from_utf8_lossy(&import.stdout).contains("imported snapshot coding"));
}
