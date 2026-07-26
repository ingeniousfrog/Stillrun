use std::path::PathBuf;

use serde_json::Value;
use stillrun::{
    db::{JobEventRecord, JobRecord, JobResourceSample, JobStatus},
    inspect,
    job_view::{
        format_job_dashboard, format_job_list, format_job_timeline, JobDashboard, JobListEntry,
        LogPreview,
    },
    jobs::status::RuntimeJobStatus,
};

#[test]
fn status_dashboard_includes_exit_reason_logs_restart_count_and_last_sample() {
    let dashboard = JobDashboard {
        job: job_record("job-1", "api", JobStatus::Failed),
        runtime: runtime(JobStatus::Failed, Some(2)),
        last_sample: Some(sample(JobStatus::Failed)),
        recent_events: vec![event("exit", "exit code 2", Some(JobStatus::Failed))],
        stdout: LogPreview {
            path: PathBuf::from("/tmp/api.out.log"),
            available: true,
            lines: vec!["listening on 3000".into()],
            error: None,
        },
        stderr: LogPreview {
            path: PathBuf::from("/tmp/api.err.log"),
            available: true,
            lines: vec!["panic: database unavailable".into()],
            error: None,
        },
    };

    let output = format_job_dashboard(&dashboard);

    assert!(output.contains("Job api (job-1)"));
    assert!(output.contains("Status: failed"));
    assert!(output.contains("Last exit: code=2"));
    assert!(output.contains("Restart count: 3"));
    assert!(output.contains(
        "Last sample: 1700000001000 status=failed pid=4242 cpu=12.5% rss=2048kb exit=2 restarts=3"
    ));
    assert!(output.contains("Recent stderr:"));
    assert!(output.contains("panic: database unavailable"));
    assert!(output.contains("Recent events:"));
    assert!(output.contains("exit code 2"));
}

#[test]
fn job_list_is_grouped_and_failed_jobs_are_highlighted() {
    let output = format_job_list(&[
        JobListEntry {
            job: job_record("job-1", "web", JobStatus::Running),
            runtime: runtime(JobStatus::Running, None),
            last_sample: Some(sample(JobStatus::Running)),
        },
        JobListEntry {
            job: job_record("job-2", "api", JobStatus::Failed),
            runtime: runtime(JobStatus::Failed, Some(2)),
            last_sample: None,
        },
        JobListEntry {
            job: job_record("job-3", "worker", JobStatus::Stopped),
            runtime: runtime(JobStatus::Stopped, Some(0)),
            last_sample: None,
        },
    ]);

    assert!(output.contains("RUNNING (1)"));
    assert!(output.contains("FAILED (1)"));
    assert!(output.contains("STOPPED (1)"));
    assert!(output.contains("! api"));
    assert!(output.find("RUNNING (1)").unwrap() < output.find("FAILED (1)").unwrap());
    assert!(output.find("FAILED (1)").unwrap() < output.find("STOPPED (1)").unwrap());
}

#[test]
fn job_events_are_rendered_as_a_readable_timeline() {
    let output = format_job_timeline(
        &job_record("job-1", "api", JobStatus::Failed),
        &[
            event("created", "job created", Some(JobStatus::Created)),
            event("started", "job started", Some(JobStatus::Running)),
            event("alert.cpu", "cpu 95.0% >= 90.0%", Some(JobStatus::Running)),
            event("exit", "exit code 2", Some(JobStatus::Failed)),
        ],
    );

    assert!(output.contains("Timeline for api (job-1)"));
    assert!(output.contains("[1700000002000] created status=created job created"));
    assert!(output.contains(
        "[1700000002000] alert.cpu status=running cpu=95.0% rss=2048kb cpu 95.0% >= 90.0%"
    ));
    assert!(output.contains(
        "[1700000002000] exit status=failed pid=4242 exit=2 cpu=95.0% rss=2048kb exit code 2"
    ));
}

#[test]
fn inspect_job_json_has_a_stable_schema_and_dashboard_fields() {
    let payload = inspect::job_payload(
        job_record("job-1", "api", JobStatus::Failed),
        runtime(JobStatus::Failed, Some(2)),
        Some(sample(JobStatus::Failed)),
        vec![event("exit", "exit code 2", Some(JobStatus::Failed))],
        LogPreview {
            path: PathBuf::from("/tmp/api.out.log"),
            available: true,
            lines: vec!["ok".into()],
            error: None,
        },
        LogPreview {
            path: PathBuf::from("/tmp/api.err.log"),
            available: true,
            lines: vec!["boom".into()],
            error: None,
        },
    );

    let json = serde_json::to_value(payload).unwrap();

    assert_eq!(json["schema_version"], Value::from(1));
    assert_eq!(json["kind"], Value::from("job"));
    assert_eq!(json["job"]["status"], Value::from("failed"));
    assert_eq!(json["runtime"]["status"], Value::from("failed"));
    assert_eq!(json["dashboard"]["restart_count"], Value::from(3));
    assert_eq!(json["dashboard"]["last_exit"]["code"], Value::from(2));
    assert_eq!(
        json["dashboard"]["recent_events"][0]["event_type"],
        Value::from("exit")
    );
    assert_eq!(
        json["dashboard"]["logs"]["stderr"]["lines"][0],
        Value::from("boom")
    );
}

fn job_record(id: &str, name: &str, status: JobStatus) -> JobRecord {
    JobRecord {
        id: id.into(),
        name: name.into(),
        label: format!("com.stillrun.{name}.1"),
        argv: vec!["sleep".into(), "10".into()],
        command: "sleep 10".into(),
        cwd: PathBuf::from("/tmp/project"),
        git_repo: None,
        git_branch: None,
        created_at_ms: 1_700_000_000_000,
        updated_at_ms: 1_700_000_002_000,
        status,
        pid: Some(4242),
        restart_count: 3,
        stdout_path: PathBuf::from(format!("/tmp/{name}.out.log")),
        stderr_path: PathBuf::from(format!("/tmp/{name}.err.log")),
        plist_path: PathBuf::from(format!("/tmp/{name}.plist")),
        keep_alive: false,
        last_exit_code: Some(2),
        last_cpu_percent: Some(12.5),
        last_rss_kb: Some(2048),
    }
}

fn runtime(status: JobStatus, exit_code: Option<i32>) -> RuntimeJobStatus {
    RuntimeJobStatus {
        status,
        pid: Some(4242),
        cpu_percent: Some(12.5),
        rss_kb: Some(2048),
        last_exit_code: exit_code,
        restart_count: Some(3),
    }
}

fn sample(status: JobStatus) -> JobResourceSample {
    JobResourceSample {
        id: 1,
        job_id: "job-1".into(),
        sampled_at_ms: 1_700_000_001_000,
        status,
        pid: Some(4242),
        last_exit_code: Some(2),
        cpu_percent: Some(12.5),
        rss_kb: Some(2048),
        restart_count: Some(3),
    }
}

fn event(event_type: &str, message: &str, status: Option<JobStatus>) -> JobEventRecord {
    JobEventRecord {
        id: 1,
        job_id: "job-1".into(),
        created_at_ms: 1_700_000_002_000,
        event_type: event_type.into(),
        message: message.into(),
        status,
        pid: Some(4242),
        last_exit_code: Some(2),
        cpu_percent: Some(95.0),
        rss_kb: Some(2048),
    }
}
