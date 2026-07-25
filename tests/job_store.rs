use std::path::PathBuf;

use stillrun::db::{JobRecord, JobRuntimeUpdate, JobStatus, Store};

#[test]
fn updates_persisted_job_runtime_status() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();

    store.upsert_job(&job_record(temp.path())).unwrap();
    let updated = store
        .update_job_runtime(
            "job-1",
            &JobRuntimeUpdate {
                status: JobStatus::Running,
                pid: Some(4242),
                last_exit_code: None,
                cpu_percent: Some(3.5),
                rss_kb: Some(2048),
                restart_count: Some(4),
                updated_at_ms: 500,
            },
        )
        .unwrap();

    assert_eq!(updated.status, JobStatus::Running);
    assert_eq!(updated.pid, Some(4242));
    assert_eq!(updated.restart_count, 4);
    assert_eq!(updated.last_cpu_percent, Some(3.5));
    assert_eq!(updated.last_rss_kb, Some(2048));
}

#[test]
fn marks_job_failed_when_launchd_reports_nonzero_exit() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();

    store.upsert_job(&job_record(temp.path())).unwrap();
    let updated = store
        .update_job_runtime(
            "job-1",
            &JobRuntimeUpdate {
                status: JobStatus::Failed,
                pid: None,
                last_exit_code: Some(2),
                cpu_percent: None,
                rss_kb: None,
                restart_count: None,
                updated_at_ms: 600,
            },
        )
        .unwrap();

    assert_eq!(updated.status, JobStatus::Failed);
    assert_eq!(updated.pid, None);
    assert_eq!(updated.last_exit_code, Some(2));
}

#[test]
fn runtime_sync_never_lowers_existing_restart_count() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();

    let job = JobRecord {
        restart_count: 5,
        ..job_record(temp.path())
    };
    store.upsert_job(&job).unwrap();
    let updated = store
        .update_job_runtime(
            "job-1",
            &JobRuntimeUpdate {
                status: JobStatus::Running,
                pid: Some(4242),
                last_exit_code: None,
                cpu_percent: None,
                rss_kb: None,
                restart_count: Some(2),
                updated_at_ms: 700,
            },
        )
        .unwrap();

    assert_eq!(updated.restart_count, 5);
}

#[test]
fn persists_keep_alive_policy_for_background_jobs() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();

    let job = JobRecord {
        keep_alive: true,
        ..job_record(temp.path())
    };
    store.upsert_job(&job).unwrap();

    let fetched = store.find_job("job-1").unwrap();
    assert!(fetched.keep_alive);
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
        stdout_path: PathBuf::from(root).join("out.log"),
        stderr_path: PathBuf::from(root).join("err.log"),
        plist_path: PathBuf::from(root).join("job.plist"),
        keep_alive: false,
        last_exit_code: None,
        last_cpu_percent: None,
        last_rss_kb: None,
    }
}
