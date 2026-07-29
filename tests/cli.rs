#![allow(clippy::expect_used)]

use std::{fs, process::Command};

use assert_cmd::cargo::CommandCargoExt;
use serde_json::Value;
use tempfile::tempdir;

fn dmlpact() -> Command {
    Command::cargo_bin("dmlpact").expect("test binary")
}

fn json_stdout(arguments: &[&str]) -> Value {
    let output = dmlpact().args(arguments).output().expect("execute dmlpact");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("valid JSON output")
}

#[test]
fn capabilities_and_contract_are_machine_readable() {
    let capabilities = json_stdout(&["capabilities"]);
    assert_eq!(capabilities["schema_version"], "dmlpact.capabilities.v1");
    assert_eq!(capabilities["safety_defaults"]["tls"], "required");
    assert_eq!(
        capabilities["safety_defaults"]["receipt_finalization_failure"],
        "exit_5_reconciliation_required"
    );

    let contract = json_stdout(&["contract"]);
    assert_eq!(contract["schema_version"], "dmlpact.contract.v1");
    assert_eq!(contract["exit_codes"]["5"], "contract_or_uncertain_commit");
}

#[test]
fn lint_emits_digests_but_not_executable_sql() {
    let directory = tempdir().expect("temporary directory");
    let sql_path = directory.path().join("change.sql");
    let raw_sql = "UPDATE accounts SET active = false WHERE id = 7";
    fs::write(&sql_path, raw_sql).expect("write SQL");

    let output = dmlpact()
        .args(["lint", "--sql"])
        .arg(&sql_path)
        .output()
        .expect("execute lint");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 stdout");
    assert!(!stdout.contains(raw_sql));
    let report: Value = serde_json::from_str(&stdout).expect("valid JSON report");
    assert_eq!(report["statement_kind"], "update");
    assert_eq!(report["executable_sql_emitted"], false);
}

#[test]
fn unsafe_sql_has_stable_policy_error_and_exit_code() {
    let directory = tempdir().expect("temporary directory");
    let sql_path = directory.path().join("unsafe.sql");
    fs::write(&sql_path, "DELETE FROM accounts").expect("write SQL");

    let output = dmlpact()
        .args(["lint", "--sql"])
        .arg(&sql_path)
        .output()
        .expect("execute lint");
    assert_eq!(output.status.code(), Some(3));
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).expect("valid JSON error");
    assert_eq!(error["schema_version"], "dmlpact.error.v1");
    assert_eq!(error["class"], "policy");
    assert_eq!(error["code"], "where_required");
}

#[test]
fn every_public_schema_and_completion_can_be_generated() {
    for document in [
        "plan",
        "receipt-event",
        "receipt-verification",
        "apply-result",
        "error",
        "capabilities",
    ] {
        let schema = json_stdout(&["schema", document]);
        assert!(schema["$schema"].is_string());
    }
    for shell in ["bash", "zsh", "fish", "powershell", "elvish"] {
        let output = dmlpact()
            .args(["completions", shell])
            .output()
            .expect("generate completion");
        assert!(output.status.success());
        assert!(!output.stdout.is_empty());
    }
}

#[test]
fn connection_material_is_not_accepted_on_argv() {
    let output = dmlpact()
        .args([
            "inspect",
            "--table",
            "accounts",
            "--database-url",
            "postgresql://user:secret@example.invalid/db",
        ])
        .output()
        .expect("execute invalid command");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 stderr");
    assert!(!stderr.contains("secret"));
    let error: Value = serde_json::from_str(&stderr).expect("valid JSON error");
    assert_eq!(error["code"], "cli_arguments_invalid");
}
