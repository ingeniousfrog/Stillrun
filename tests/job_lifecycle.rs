use stillrun::{
    db::{JobRecord, JobStatus},
    jobs::status::RuntimeJobStatus,
    jobs::{
        format_launchd_operation_error, is_already_unloaded_bootout_error,
        start_action_for_loaded_runtime_status, LaunchdStartAction,
    },
    StillrunError,
};

use std::path::PathBuf;

#[test]
fn classifies_launchd_bootout_not_found_as_already_stopped() {
    assert!(is_already_unloaded_bootout_error(
        r#"Boot-out failed: 3: No such process"#
    ));
    assert!(is_already_unloaded_bootout_error(
        r#"Could not find service "com.stillrun.dev" in domain for user gui: 501"#
    ));
}

#[test]
fn does_not_hide_unrelated_launchd_bootout_errors() {
    assert!(!is_already_unloaded_bootout_error(
        r#"Boot-out failed: 5: Input/output error"#
    ));
}

#[test]
fn running_loaded_job_start_is_idempotent() {
    let runtime = runtime_status(JobStatus::Running);

    assert_eq!(
        start_action_for_loaded_runtime_status(&runtime),
        LaunchdStartAction::AlreadyRunning
    );
}

#[test]
fn stopped_loaded_job_start_uses_kickstart() {
    let runtime = runtime_status(JobStatus::Stopped);

    assert_eq!(
        start_action_for_loaded_runtime_status(&runtime),
        LaunchdStartAction::Kickstart
    );
}

#[test]
fn launchd_operation_errors_name_job_and_recovery_commands() {
    let message = format_launchd_operation_error(
        "stop before restart",
        &job_record(),
        StillrunError::invalid("launchctl bootout failed: Input/output error"),
    );

    assert!(message.contains("could not stop before restart job 'dev' (job-1) via launchd"));
    assert!(message.contains("plist=/tmp/dev.plist"));
    assert!(message.contains("stillrun status dev"));
    assert!(message.contains("stillrun jobs delete dev"));
}

fn runtime_status(status: JobStatus) -> RuntimeJobStatus {
    RuntimeJobStatus {
        status,
        pid: None,
        cpu_percent: None,
        rss_kb: None,
        last_exit_code: None,
        restart_count: None,
    }
}

fn job_record() -> JobRecord {
    JobRecord {
        id: "job-1".into(),
        name: "dev".into(),
        label: "com.stillrun.dev.1".into(),
        argv: vec!["sleep".into(), "10".into()],
        command: "sleep 10".into(),
        cwd: PathBuf::from("/tmp"),
        git_repo: None,
        git_branch: None,
        created_at_ms: 1,
        updated_at_ms: 1,
        status: JobStatus::Running,
        pid: Some(123),
        restart_count: 0,
        stdout_path: PathBuf::from("/tmp/dev.out.log"),
        stderr_path: PathBuf::from("/tmp/dev.err.log"),
        plist_path: PathBuf::from("/tmp/dev.plist"),
        keep_alive: false,
        last_exit_code: None,
        last_cpu_percent: None,
        last_rss_kb: None,
    }
}
