use stillrun::{
    db::{JobRecord, JobStatus},
    jobs::status::RuntimeJobStatus,
    jobs::{
        format_launchd_operation_error, is_already_unloaded_bootout_error,
        start_action_for_loaded_runtime_status, validate_background_argv_is_safe,
        LaunchdStartAction,
    },
    redact::RedactionPolicy,
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

#[test]
fn background_jobs_reject_sensitive_argv_before_writing_launchd_plists() {
    let error = validate_background_argv_is_safe(
        &["curl".into(), "--token".into(), "secret-token".into()],
        &RedactionPolicy::default(),
    )
    .unwrap_err()
    .to_string();

    assert!(error.contains("sensitive command argument"));
    assert!(error.contains("launchd plist"));
}

#[test]
fn monitor_job_argv_runs_foreground_monitor_loop_under_launchd() {
    let argv = stillrun::jobs::build_monitor_job_argv(
        &PathBuf::from("/usr/local/bin/stillrun"),
        "dev",
        2,
        Some(80.0),
        Some(512),
    );

    assert_eq!(
        argv,
        vec![
            "/usr/local/bin/stillrun",
            "jobs",
            "monitor",
            "dev",
            "--interval-secs",
            "2",
            "--cpu-alert",
            "80",
            "--rss-alert-mb",
            "512",
        ]
    );
    assert!(!argv.iter().any(|arg| arg == "--background"));
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
