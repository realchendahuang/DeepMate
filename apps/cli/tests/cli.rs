use std::process::Command;

fn deepmate(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_deepmate"))
        .args(args)
        .output()
        .expect("failed to run deepmate binary")
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
