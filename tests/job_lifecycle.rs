use stillrun::{
    db::JobStatus,
    jobs::status::RuntimeJobStatus,
    jobs::{
        is_already_unloaded_bootout_error, start_action_for_loaded_runtime_status,
        LaunchdStartAction,
    },
};

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
