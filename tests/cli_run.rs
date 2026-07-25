use std::fs;

use assert_cmd::Command;
use predicates::str::contains;
use stillrun::db::{JobRecord, JobStatus, Store};

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
        .stdout(contains("job-1"))
        .stdout(contains("runtime="))
        .stdout(contains("label: com.stillrun.dev.1"))
        .stdout(contains("stdout:"));
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
        ])
        .assert()
        .success()
        .stdout(contains("imported=0"))
        .stdout(contains("skipped=1"));
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
