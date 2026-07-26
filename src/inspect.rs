use serde::Serialize;

use crate::{
    db::{ExecutionRecord, JobEventRecord, JobRecord, JobResourceSample},
    job_view::LogPreview,
    jobs::status::RuntimeJobStatus,
};

const INSPECT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum InspectPayload {
    Execution(Box<ExecutionInspectPayload>),
    Job(Box<JobInspectPayload>),
}

#[derive(Debug, Serialize)]
pub struct ExecutionInspectPayload {
    pub schema_version: u32,
    pub kind: &'static str,
    pub execution: ExecutionJson,
}

#[derive(Debug, Serialize)]
pub struct JobInspectPayload {
    pub schema_version: u32,
    pub kind: &'static str,
    pub job: JobJson,
    pub runtime: RuntimeJson,
    pub dashboard: JobDashboardJson,
}

#[derive(Debug, Serialize)]
pub struct ExecutionJson {
    pub id: i64,
    pub command: String,
    pub argv: Vec<String>,
    pub cwd: String,
    pub git_repo: Option<String>,
    pub git_branch: Option<String>,
    pub started_at_ms: i64,
    pub ended_at_ms: Option<i64>,
    pub duration_ms: Option<i64>,
    pub exit_code: Option<i32>,
    pub status: &'static str,
    pub stdout_bytes: usize,
    pub stderr_bytes: usize,
    pub pid: Option<u32>,
    pub background_job_id: Option<String>,
    pub restart_count: i64,
    pub source: String,
    pub source_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct JobJson {
    pub id: String,
    pub name: String,
    pub label: String,
    pub argv: Vec<String>,
    pub command: String,
    pub cwd: String,
    pub git_repo: Option<String>,
    pub git_branch: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub status: &'static str,
    pub pid: Option<u32>,
    pub restart_count: i64,
    pub stdout_path: String,
    pub stderr_path: String,
    pub plist_path: String,
    pub keep_alive: bool,
    pub last_exit_code: Option<i32>,
    pub last_cpu_percent: Option<f32>,
    pub last_rss_kb: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct RuntimeJson {
    pub status: &'static str,
    pub pid: Option<u32>,
    pub cpu_percent: Option<f32>,
    pub rss_kb: Option<u64>,
    pub last_exit_code: Option<i32>,
    pub restart_count: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct JobDashboardJson {
    pub restart_count: i64,
    pub last_exit: LastExitJson,
    pub last_sample: Option<JobResourceSampleJson>,
    pub recent_events: Vec<JobEventJson>,
    pub logs: JobLogsJson,
}

#[derive(Debug, Serialize)]
pub struct LastExitJson {
    pub code: Option<i32>,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct JobResourceSampleJson {
    pub id: i64,
    pub job_id: String,
    pub sampled_at_ms: i64,
    pub status: &'static str,
    pub pid: Option<u32>,
    pub last_exit_code: Option<i32>,
    pub cpu_percent: Option<f32>,
    pub rss_kb: Option<u64>,
    pub restart_count: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct JobEventJson {
    pub id: i64,
    pub job_id: String,
    pub created_at_ms: i64,
    pub event_type: String,
    pub message: String,
    pub status: Option<&'static str>,
    pub pid: Option<u32>,
    pub last_exit_code: Option<i32>,
    pub cpu_percent: Option<f32>,
    pub rss_kb: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct JobLogsJson {
    pub stdout: LogPreview,
    pub stderr: LogPreview,
}

impl From<ExecutionRecord> for ExecutionJson {
    fn from(record: ExecutionRecord) -> Self {
        Self {
            id: record.id,
            command: record.command,
            argv: record.argv,
            cwd: record.cwd.display().to_string(),
            git_repo: record.git_repo.map(|path| path.display().to_string()),
            git_branch: record.git_branch,
            started_at_ms: record.started_at_ms,
            ended_at_ms: record.ended_at_ms,
            duration_ms: record.duration_ms,
            exit_code: record.exit_code,
            status: record.status.as_str(),
            stdout_bytes: record.stdout.len(),
            stderr_bytes: record.stderr.len(),
            pid: record.pid,
            background_job_id: record.background_job_id,
            restart_count: record.restart_count,
            source: record.source,
            source_id: record.source_id,
        }
    }
}

impl From<JobRecord> for JobJson {
    fn from(job: JobRecord) -> Self {
        Self {
            id: job.id,
            name: job.name,
            label: job.label,
            argv: job.argv,
            command: job.command,
            cwd: job.cwd.display().to_string(),
            git_repo: job.git_repo.map(|path| path.display().to_string()),
            git_branch: job.git_branch,
            created_at_ms: job.created_at_ms,
            updated_at_ms: job.updated_at_ms,
            status: job.status.as_str(),
            pid: job.pid,
            restart_count: job.restart_count,
            stdout_path: job.stdout_path.display().to_string(),
            stderr_path: job.stderr_path.display().to_string(),
            plist_path: job.plist_path.display().to_string(),
            keep_alive: job.keep_alive,
            last_exit_code: job.last_exit_code,
            last_cpu_percent: job.last_cpu_percent,
            last_rss_kb: job.last_rss_kb,
        }
    }
}

impl From<RuntimeJobStatus> for RuntimeJson {
    fn from(runtime: RuntimeJobStatus) -> Self {
        Self {
            status: runtime.status.as_str(),
            pid: runtime.pid,
            cpu_percent: runtime.cpu_percent,
            rss_kb: runtime.rss_kb,
            last_exit_code: runtime.last_exit_code,
            restart_count: runtime.restart_count,
        }
    }
}

impl From<JobResourceSample> for JobResourceSampleJson {
    fn from(sample: JobResourceSample) -> Self {
        Self {
            id: sample.id,
            job_id: sample.job_id,
            sampled_at_ms: sample.sampled_at_ms,
            status: sample.status.as_str(),
            pid: sample.pid,
            last_exit_code: sample.last_exit_code,
            cpu_percent: sample.cpu_percent,
            rss_kb: sample.rss_kb,
            restart_count: sample.restart_count,
        }
    }
}

impl From<JobEventRecord> for JobEventJson {
    fn from(event: JobEventRecord) -> Self {
        Self {
            id: event.id,
            job_id: event.job_id,
            created_at_ms: event.created_at_ms,
            event_type: event.event_type,
            message: event.message,
            status: event.status.map(|status| status.as_str()),
            pid: event.pid,
            last_exit_code: event.last_exit_code,
            cpu_percent: event.cpu_percent,
            rss_kb: event.rss_kb,
        }
    }
}

pub fn execution_payload(record: ExecutionRecord) -> InspectPayload {
    InspectPayload::Execution(Box::new(ExecutionInspectPayload {
        schema_version: INSPECT_SCHEMA_VERSION,
        kind: "execution",
        execution: record.into(),
    }))
}

pub fn job_payload(
    job: JobRecord,
    runtime: RuntimeJobStatus,
    last_sample: Option<JobResourceSample>,
    recent_events: Vec<JobEventRecord>,
    stdout: LogPreview,
    stderr: LogPreview,
) -> InspectPayload {
    let restart_count = runtime.restart_count.unwrap_or(job.restart_count);
    let last_exit = LastExitJson {
        code: runtime.last_exit_code.or(job.last_exit_code),
        reason: recent_events
            .iter()
            .find(|event| event.event_type == "exit")
            .map(|event| event.message.clone()),
    };
    InspectPayload::Job(Box::new(JobInspectPayload {
        schema_version: INSPECT_SCHEMA_VERSION,
        kind: "job",
        job: job.into(),
        runtime: runtime.into(),
        dashboard: JobDashboardJson {
            restart_count,
            last_exit,
            last_sample: last_sample.map(Into::into),
            recent_events: recent_events.into_iter().map(Into::into).collect(),
            logs: JobLogsJson { stdout, stderr },
        },
    }))
}

pub fn format_execution_inspect(record: &ExecutionRecord) -> String {
    let mut output = String::new();
    output.push_str(&format!("Execution #{}\n", record.id));
    output.push_str(&format!("  status: {}\n", record.status.as_str()));
    output.push_str(&format!("  command: {}\n", record.command));
    output.push_str(&format!("  cwd: {}\n", record.cwd.display()));
    if let Some(repo) = &record.git_repo {
        output.push_str(&format!("  git repo: {}\n", repo.display()));
    }
    if let Some(branch) = &record.git_branch {
        output.push_str(&format!("  git branch: {branch}\n"));
    }
    output.push_str(&format!("  source: {}", record.source));
    if let Some(source_id) = &record.source_id {
        output.push_str(&format!(":{source_id}"));
    }
    output.push('\n');
    output.push_str(&format!("  started ms: {}\n", record.started_at_ms));
    output.push_str(&format!("  ended ms: {:?}\n", record.ended_at_ms));
    output.push_str(&format!("  exit code: {:?}\n", record.exit_code));
    output.push_str(&format!("  duration ms: {:?}\n", record.duration_ms));
    output.push_str(&format!("  pid: {:?}\n", record.pid));
    output.push_str(&format!("  stdout bytes: {}\n", record.stdout.len()));
    output.push_str(&format!("  stderr bytes: {}\n", record.stderr.len()));
    output
}

pub fn format_replay_preview(record: &ExecutionRecord) -> String {
    let mut output = String::new();
    output.push_str(&format!("Replay preview #{}\n", record.id));
    output.push_str(&format!("  command: {}\n", record.command));
    output.push_str(&format!("  cwd: {}\n", record.cwd.display()));
    output.push_str(&format!("  status: {}\n", record.status.as_str()));
    output.push_str(&format!("  source: {}", record.source));
    if let Some(source_id) = &record.source_id {
        output.push_str(&format!(":{source_id}"));
    }
    output.push('\n');
    if let Some(repo) = &record.git_repo {
        output.push_str(&format!("  git repo: {}\n", repo.display()));
    }
    if let Some(branch) = &record.git_branch {
        output.push_str(&format!("  git branch: {branch}\n"));
    }
    output
}
