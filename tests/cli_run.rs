use std::fs;

use assert_cmd::Command;
use predicates::str::{contains, is_empty};
use stillrun::{
    context::CommandContext,
    db::{ExecutionStatus, HistoryFilter, JobRecord, JobStatus, NewExecution, Store},
};

#[test]
fn cli_short_help_shows_capabilities_and_examples() {
    Command::cargo_bin("stillrun")
        .unwrap()
        .args(["-h"])
        .assert()
        .success()
        .stdout(contains("Capabilities"))
        .stdout(contains("Examples"))
        .stdout(contains("Run and record"))
        .stdout(contains("stillrun run -- npm run dev"))
        .stdout(contains("stillrun run --shell"))
        .stdout(contains(
            "stillrun history --query \"npm\" --since 7d --json",
        ));
}

#[test]
fn cli_version_reports_package_version() {
    Command::cargo_bin("stillrun")
        .unwrap()
        .args(["--version"])
        .assert()
        .success()
        .stdout(contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn cli_static_completion_does_not_open_local_state() {
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("not-a-directory");
    fs::write(&state_file, "state path collision").unwrap();

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", state_file)
        .args(["completion", "zsh"])
        .assert()
        .success()
        .stdout(contains("#compdef stillrun"));
}

#[test]
fn cli_run_records_command_and_history_finds_it() {
    let temp = tempfile::tempdir().unwrap();

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["run", "--", "printf", "stillrun-cli"])
        .assert()
        .success()
        .stdout(contains("stillrun-cli"));

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["history", "--query", "stillrun-cli"])
        .assert()
        .success()
        .stdout(contains("stillrun-cli"));
}

#[test]
fn cli_replay_uses_original_command_and_cwd() {
    let temp = tempfile::tempdir().unwrap();

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["run", "--", "printf", "replay-me"])
        .assert()
        .success();

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["replay", "1"])
        .assert()
        .success()
        .stdout(contains("replay-me"));
}

#[test]
fn cli_run_shell_wraps_complex_command_for_replayable_history() {
    let temp = tempfile::tempdir().unwrap();

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .env("SHELL", "/bin/sh")
        .args(["run", "--shell", "printf shell-ok"])
        .assert()
        .success()
        .stdout(contains("shell-ok"));

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["history", "--query", "shell-ok", "--full"])
        .assert()
        .success()
        .stdout(contains("/bin/sh -lc 'printf shell-ok'"));
}

#[test]
fn cli_configured_redact_keys_apply_to_persisted_history() {
    let temp = tempfile::tempdir().unwrap();

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["config", "redact", "add", "customsecret"])
        .assert()
        .success();

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .env("CUSTOMSECRET", "env-sentinel")
        .args([
            "run",
            "--",
            "/bin/sh",
            "-c",
            "printf '%s' \"$CUSTOMSECRET\"; printf '%s' ' --customsecret arg-sentinel'",
        ])
        .assert()
        .success()
        .stdout(contains(stillrun::redact::REDACTED));

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["history", "--query", "sentinel", "--full", "--details"])
        .assert()
        .success()
        .stdout(is_empty());

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["history", "--query", stillrun::redact::REDACTED, "--full"])
        .assert()
        .success()
        .stdout(contains(stillrun::redact::REDACTED));
}

#[test]
fn cli_replay_does_not_inherit_new_environment_values() {
    let temp = tempfile::tempdir().unwrap();

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .env_remove("STILLRUN_REPLAY_LEAK")
        .args([
            "run",
            "--",
            "/bin/sh",
            "-c",
            "printf '%s' \"${STILLRUN_REPLAY_LEAK:-unset}\"",
        ])
        .assert()
        .success()
        .stdout(contains("unset"));

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .env("STILLRUN_REPLAY_LEAK", "leaked")
        .args(["replay", "1"])
        .assert()
        .success()
        .stdout(contains("unset"));
}

#[test]
fn cli_replay_preview_explains_environment_and_context_limits() {
    let temp = tempfile::tempdir().unwrap();

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .env("SAFE_REPLAY_FLAG", "yes")
        .env("API_TOKEN", "secret-token")
        .args(["run", "--", "printf", "preview-env"])
        .assert()
        .success();

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["replay", "1", "--preview"])
        .assert()
        .success()
        .stdout(contains("restorable env:"))
        .stdout(contains("redacted env omitted:"))
        .stdout(contains("git head:"))
        .stdout(contains("does not checkout git state"));
}

#[test]
fn cli_status_prints_job_runtime_summary() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();
    store.upsert_job(&job_record(temp.path())).unwrap();

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["status", "dev"])
        .assert()
        .success()
        .stdout(contains("Job dev (job-1)"))
        .stdout(contains("Status:"))
        .stdout(contains("Restart count:"))
        .stdout(contains("Last sample:"))
        .stdout(contains("Recent stdout:"))
        .stdout(contains("Label: com.stillrun.dev.1"));
}

#[test]
fn cli_jobs_monitor_records_samples_and_events() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();
    store.upsert_job(&job_record(temp.path())).unwrap();

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["jobs", "monitor", "dev", "--once"])
        .assert()
        .success()
        .stdout(contains("Job dev (job-1)"))
        .stdout(contains("Last sample:"))
        .stdout(contains("Alerts: 0"));

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["jobs", "samples", "dev"])
        .assert()
        .success()
        .stdout(contains("job=job-1"));

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["jobs", "events", "dev"])
        .assert()
        .success()
        .stdout(contains("Timeline for dev (job-1)"))
        .stdout(contains("status status="));
}

#[test]
fn cli_history_maintenance_commands_delete_clear_and_prune() {
    let temp = tempfile::tempdir().unwrap();
    let history_path = temp.path().join("zsh_history");
    fs::write(&history_path, ": 1700000000:0;npm run imported-clear\n").unwrap();

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["run", "--", "printf", "delete-cli"])
        .assert()
        .success();

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["history", "--query", "delete-cli"])
        .assert()
        .success()
        .stdout(contains("delete-cli"));

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["history", "delete", "1"])
        .assert()
        .success()
        .stdout(contains("deleted history #1"));

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["history", "--query", "delete-cli"])
        .assert()
        .success()
        .stdout(is_empty());

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .env("HOME", temp.path())
        .args([
            "import-history",
            "--shell",
            "zsh",
            "--file",
            history_path.to_str().unwrap(),
            "--yes",
        ])
        .assert()
        .success();

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["history", "clear", "--imported", "--yes"])
        .assert()
        .success()
        .stdout(contains("deleted=1"));

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["history", "prune", "--before-ms", "9999999999999", "--yes"])
        .assert()
        .success()
        .stdout(contains("deleted=0"));
}

#[test]
fn cli_jobs_delete_removes_job_record_and_plist() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();
    let job = job_record(temp.path());
    fs::write(&job.plist_path, "<plist/>").unwrap();
    store.upsert_job(&job).unwrap();

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["jobs", "delete", "dev"])
        .assert()
        .success()
        .stdout(contains("deleted job job-1"));

    assert!(store.find_job("job-1").is_err());
    assert!(!job.plist_path.exists());
}

#[test]
fn cli_import_history_makes_shell_history_searchable() {
    let temp = tempfile::tempdir().unwrap();
    let history_path = temp.path().join("zsh_history");
    fs::write(&history_path, ": 1700000000:0;npm run cli-import\n").unwrap();

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .env("HOME", temp.path())
        .args([
            "import-history",
            "--shell",
            "zsh",
            "--file",
            history_path.to_str().unwrap(),
            "--yes",
        ])
        .assert()
        .success()
        .stdout(contains("imported=1"))
        .stdout(contains("skipped=0"));

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .env("HOME", temp.path())
        .args(["history", "--query", "cli-import"])
        .assert()
        .success()
        .stdout(contains("npm run cli-import"));

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .env("HOME", temp.path())
        .args([
            "import-history",
            "--shell",
            "zsh",
            "--file",
            history_path.to_str().unwrap(),
            "--yes",
        ])
        .assert()
        .success()
        .stdout(contains("imported=0"))
        .stdout(contains("skipped=1"));
}

#[test]
fn cli_import_history_previews_and_requires_confirmation() {
    let temp = tempfile::tempdir().unwrap();
    let history_path = temp.path().join("zsh_history");
    fs::write(&history_path, ": 1700000000:0;npm run preview-import\n").unwrap();

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .env("HOME", temp.path())
        .args([
            "import-history",
            "--shell",
            "zsh",
            "--file",
            history_path.to_str().unwrap(),
            "--preview",
        ])
        .assert()
        .success()
        .stdout(contains("would_import=1"))
        .stdout(contains("zsh_history"));

    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();
    assert!(store
        .search_history(&HistoryFilter {
            query: Some("preview-import".into()),
            limit: 10,
            ..HistoryFilter::default()
        })
        .unwrap()
        .is_empty());

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .env("HOME", temp.path())
        .args([
            "import-history",
            "--shell",
            "zsh",
            "--file",
            history_path.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(contains("requires confirmation"));
}

#[test]
fn cli_replay_imported_history_requires_preview_or_confirmation() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();
    let id = store
        .insert_imported_execution(
            &NewExecution {
                argv: vec![
                    "/bin/sh".into(),
                    "-c".into(),
                    "printf replay-imported".into(),
                ],
                context: CommandContext {
                    cwd: temp.path().to_path_buf(),
                    git_repo: None,
                    git_branch: None,
                    git_head: None,
                    env: Default::default(),
                },
                started_at_ms: 1_700_000_000_000,
                ended_at_ms: None,
                duration_ms: None,
                exit_code: None,
                status: ExecutionStatus::Imported,
                stdout: String::new(),
                stderr: String::new(),
                pid: None,
                background_job_id: None,
                restart_count: 0,
            },
            "shell:zsh:/tmp/history",
            "1",
            "printf replay-imported",
        )
        .unwrap()
        .unwrap();

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["replay", &id.to_string(), "--preview"])
        .assert()
        .success()
        .stdout(contains("Replay preview"))
        .stdout(contains("printf replay-imported"))
        .stdout(contains("source: shell:zsh:/tmp/history:1"));

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["replay", &id.to_string()])
        .assert()
        .failure()
        .stderr(contains("requires confirmation"));

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["replay", &id.to_string(), "--yes"])
        .assert()
        .success()
        .stdout(contains("replay-imported"));
}

#[test]
fn cli_history_sort_controls_display_order() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();
    insert_test_execution(&store, temp.path(), "old-cli", 1_000);
    insert_test_execution(&store, temp.path(), "new-cli", 2_000);

    let output = Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["history", "--sort", "oldest", "--full"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();

    assert!(text.find("old-cli").unwrap() < text.find("new-cli").unwrap());
}

#[test]
fn cli_history_json_supports_exit_branch_and_since_filters() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();
    store
        .insert_execution(&NewExecution {
            argv: vec!["sh".into(), "-c".into(), "exit 2".into()],
            context: CommandContext {
                cwd: temp.path().to_path_buf(),
                git_repo: Some(temp.path().to_path_buf()),
                git_branch: Some("main".into()),
                git_head: Some("abc123".into()),
                env: Default::default(),
            },
            started_at_ms: 2_000,
            ended_at_ms: Some(2_010),
            duration_ms: Some(10),
            exit_code: Some(2),
            status: ExecutionStatus::Failed,
            stdout: String::new(),
            stderr: "failed-json".into(),
            pid: None,
            background_job_id: None,
            restart_count: 0,
        })
        .unwrap();

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args([
            "history",
            "--since",
            "1000",
            "--exit-code",
            "2",
            "--branch",
            "main",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains(r#""command":"sh -c 'exit 2'""#))
        .stdout(contains(r#""git_branch":"main""#))
        .stdout(contains(r#""exit_code":2"#));
}

#[test]
fn cli_inspect_json_outputs_structured_execution_payload() {
    let temp = tempfile::tempdir().unwrap();

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["run", "--", "printf", "inspect-json"])
        .assert()
        .success();

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["inspect", "1", "--json"])
        .assert()
        .success()
        .stdout(contains(r#""schema_version":1"#))
        .stdout(contains(r#""kind":"execution""#))
        .stdout(contains(r#""execution":{"id":1"#))
        .stdout(contains(r#""command":"printf inspect-json""#));
}

#[test]
fn cli_config_manages_persisted_config_file() {
    let temp = tempfile::tempdir().unwrap();

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["config", "set", "max-output-bytes", "4096"])
        .assert()
        .success()
        .stdout(contains("max_output_bytes=4096"));

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["config", "redact", "add", "session_token"])
        .assert()
        .success()
        .stdout(contains("added redact key session_token"));

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["config", "show", "--json"])
        .assert()
        .success()
        .stdout(contains(r#""max_output_bytes":4096"#))
        .stdout(contains("session_token"));

    assert!(temp.path().join("config.toml").exists());
}

#[test]
fn cli_completion_scripts_and_job_candidates_are_available() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();
    store.upsert_job(&job_record(temp.path())).unwrap();

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["completion", "zsh"])
        .assert()
        .success()
        .stdout(contains("completion candidates jobs"));

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["completion", "candidates", "jobs", "--prefix", "de"])
        .assert()
        .success()
        .stdout(contains("dev"));

    Command::cargo_bin("stillrun")
        .unwrap()
        .env("STILLRUN_HOME", temp.path())
        .args(["completion", "candidates", "jobs"])
        .assert()
        .success()
        .stdout(contains("job-1"));
}

fn job_record(root: &std::path::Path) -> JobRecord {
    JobRecord {
        id: "job-1".into(),
        name: "dev".into(),
        label: "com.stillrun.dev.1".into(),
        argv: vec!["sleep".into(), "10".into()],
        command: "sleep 10".into(),
        cwd: root.to_path_buf(),
        git_repo: None,
        git_branch: None,
        created_at_ms: 100,
        updated_at_ms: 100,
        status: JobStatus::Created,
        pid: None,
        restart_count: 0,
        stdout_path: root.join("out.log"),
        stderr_path: root.join("err.log"),
        plist_path: root.join("job.plist"),
        keep_alive: false,
        last_exit_code: None,
        last_cpu_percent: None,
        last_rss_kb: None,
    }
}

fn insert_test_execution(store: &Store, cwd: &std::path::Path, label: &str, started_at_ms: i64) {
    store
        .insert_execution(&NewExecution {
            argv: vec!["echo".into(), label.into()],
            context: CommandContext {
                cwd: cwd.to_path_buf(),
                git_repo: None,
                git_branch: None,
                git_head: None,
                env: Default::default(),
            },
            started_at_ms,
            ended_at_ms: Some(started_at_ms + 1),
            duration_ms: Some(1),
            exit_code: Some(0),
            status: ExecutionStatus::Success,
            stdout: String::new(),
            stderr: String::new(),
            pid: None,
            background_job_id: None,
            restart_count: 0,
        })
        .unwrap();
}
