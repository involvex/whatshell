use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn help_lists_whatshell_commands() {
    let mut cmd = Command::cargo_bin("whatshell").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("messages"))
        .stdout(predicate::str::contains("send"))
        .stdout(predicate::str::contains("contacts"))
        .stdout(predicate::str::contains("export"));
}

#[test]
fn dry_run_send_text_is_json_and_does_not_connect() {
    let dir = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("whatshell").unwrap();
    cmd.args([
        "--json",
        "--store",
        dir.path().to_str().unwrap(),
        "send",
        "text",
        "--to",
        "+15551234567",
        "--message",
        "hello",
        "--dry-run",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("\"success\": true"))
    .stdout(predicate::str::contains("15551234567@s.whatsapp.net"))
    .stdout(predicate::str::contains("\"dry_run\": true"));
}

#[test]
fn dry_run_send_poll_is_json_and_does_not_connect() {
    let dir = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("whatshell").unwrap();
    cmd.args([
        "--json",
        "--store",
        dir.path().to_str().unwrap(),
        "send",
        "poll",
        "--to",
        "+15551234567",
        "--question",
        "Ship?",
        "--option",
        "Yes",
        "--option",
        "No",
        "--dry-run",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("\"kind\": \"poll\""))
    .stdout(predicate::str::contains("\"dry_run\": true"));
}

#[test]
fn dry_run_send_location_is_json_and_does_not_connect() {
    let dir = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("whatshell").unwrap();
    cmd.args([
        "--json",
        "--store",
        dir.path().to_str().unwrap(),
        "send",
        "location",
        "--to",
        "+15551234567",
        "--latitude",
        "12.9716",
        "--longitude",
        "77.5946",
        "--dry-run",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("\"kind\": \"location\""))
    .stdout(predicate::str::contains("\"dry_run\": true"));
}

#[test]
fn auth_status_reports_missing_session_without_creating_one() {
    let dir = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("whatshell").unwrap();
    cmd.args([
        "--json",
        "--store",
        dir.path().to_str().unwrap(),
        "auth",
        "status",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("\"authenticated\": false"));
}

#[test]
fn completions_bash_prints_script() {
    let mut cmd = Command::cargo_bin("whatshell").unwrap();
    cmd.args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("whatshell"));
}

#[test]
fn completions_pwsh_prints_script() {
    let mut cmd = Command::cargo_bin("whatshell").unwrap();
    cmd.args(["completions", "pwsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("whatshell"));
}

#[test]
fn completions_install_dry_run_does_not_write() {
    let dir = tempfile::tempdir().unwrap();
    let bashrc = dir.path().join(".bashrc");
    let mut cmd = Command::cargo_bin("whatshell").unwrap();
    cmd.env("HOME", dir.path())
        .env("USERPROFILE", dir.path())
        .args(["--json", "completions", "bash", "--install", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"dry_run\": true"))
        .stdout(predicate::str::contains("\"installed\": false"))
        .stdout(predicate::str::contains("whatshell completions bash"));
    assert!(!bashrc.exists());
}

#[test]
fn completions_install_writes_managed_block() {
    let dir = tempfile::tempdir().unwrap();
    let bashrc = dir.path().join(".bashrc");
    let mut cmd = Command::cargo_bin("whatshell").unwrap();
    cmd.env("HOME", dir.path())
        .env("USERPROFILE", dir.path())
        .args(["--json", "completions", "bash", "--install"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"installed\": true"));
    let contents = std::fs::read_to_string(&bashrc).unwrap();
    assert!(contents.contains("# >>> whatshell completions >>>"));
    assert!(contents.contains("eval \"$(whatshell completions bash)\""));
}

#[test]
fn bare_json_without_subcommand_errors() {
    let dir = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("whatshell").unwrap();
    cmd.args(["--json", "--store", dir.path().to_str().unwrap()])
        .assert()
        .failure()
        .stdout(predicate::str::contains("no subcommand"));
}

#[test]
fn help_lists_completions() {
    let mut cmd = Command::cargo_bin("whatshell").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("completions"));
}
