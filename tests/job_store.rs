use std::path::PathBuf;

use stillrun::db::{
    JobRecord, JobRuntimeUpdate, JobStatus, NewJobEvent, NewJobResourceSample, Store,
};
use stillrun::jobs::{self, status::RuntimeJobStatus};

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

#[test]
fn deletes_job_record_by_id() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();

    store.upsert_job(&job_record(temp.path())).unwrap();

    assert!(store.delete_job_record("job-1").unwrap());
    assert!(!store.delete_job_record("job-1").unwrap());
    assert!(store.find_job("job-1").is_err());
}

#[test]
fn records_and_lists_job_resource_samples() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();
    store.upsert_job(&job_record(temp.path())).unwrap();

    store
        .record_job_resource_sample(&NewJobResourceSample {
            job_id: "job-1".into(),
            sampled_at_ms: 900,
            status: JobStatus::Running,
            pid: Some(123),
            last_exit_code: None,
            cpu_percent: Some(12.5),
            rss_kb: Some(4096),
            restart_count: Some(2),
        })
        .unwrap();

    let samples = store.list_job_resource_samples("job-1", 10).unwrap();

    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].status, JobStatus::Running);
    assert_eq!(samples[0].cpu_percent, Some(12.5));
    assert_eq!(samples[0].rss_kb, Some(4096));
}

#[test]
fn records_and_lists_job_events_after_cursor() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();
    store.upsert_job(&job_record(temp.path())).unwrap();

    let first = store
        .record_job_event(&NewJobEvent {
            job_id: "job-1".into(),
            created_at_ms: 1_000,
            event_type: "status".into(),
            message: "status created -> running".into(),
            status: Some(JobStatus::Running),
            pid: Some(123),
            last_exit_code: None,
            cpu_percent: Some(1.0),
            rss_kb: Some(128),
        })
        .unwrap();
    store
        .record_job_event(&NewJobEvent {
            job_id: "job-1".into(),
            created_at_ms: 1_001,
            event_type: "alert.cpu".into(),
            message: "cpu 91.0% >= 90.0%".into(),
            status: Some(JobStatus::Running),
            pid: Some(123),
            last_exit_code: None,
            cpu_percent: Some(91.0),
            rss_kb: Some(128),
        })
        .unwrap();

    let events = store
        .list_job_events_after("job-1", Some(first), 10)
        .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, "alert.cpu");
}

#[test]
fn runtime_sync_records_sample_and_change_event() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();
    store.upsert_job(&job_record(temp.path())).unwrap();
    let job = store.find_job("job-1").unwrap();

    jobs::sync_job_runtime_status(
        &store,
        &job,
        &RuntimeJobStatus {
            status: JobStatus::Running,
            pid: Some(456),
            cpu_percent: Some(22.0),
            rss_kb: Some(8192),
            last_exit_code: None,
            restart_count: Some(3),
        },
    )
    .unwrap();

    let samples = store.list_job_resource_samples("job-1", 10).unwrap();
    let events = store.list_job_events("job-1", 10).unwrap();

    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].pid, Some(456));
    assert!(events.iter().any(|event| event.event_type == "status"));
    assert!(events.iter().any(|event| event.event_type == "restart"));
}

#[test]
fn resource_alerts_are_recorded_as_runtime_events() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();
    store.upsert_job(&job_record(temp.path())).unwrap();
    let job = store.find_job("job-1").unwrap();

    let recorded = jobs::record_resource_alerts(
        &store,
        &job,
        &RuntimeJobStatus {
            status: JobStatus::Running,
            pid: Some(789),
            cpu_percent: Some(95.0),
            rss_kb: Some(600 * 1024),
            last_exit_code: None,
            restart_count: Some(0),
        },
        Some(90.0),
        Some(512 * 1024),
    )
    .unwrap();

    let events = store.list_job_events("job-1", 10).unwrap();

    assert_eq!(recorded, 2);
    assert!(events.iter().any(|event| event.event_type == "alert.cpu"));
    assert!(events.iter().any(|event| event.event_type == "alert.rss"));
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
