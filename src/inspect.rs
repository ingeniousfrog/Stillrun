use serde::Serialize;

use crate::{
    db::{ExecutionRecord, JobRecord},
    jobs::status::RuntimeJobStatus,
};

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InspectPayload {
    Execution {
        record: ExecutionRecord,
    },
    Job {
        job: JobRecord,
        runtime: Option<RuntimeJobStatus>,
    },
}

pub fn execution_payload(record: ExecutionRecord) -> InspectPayload {
    InspectPayload::Execution { record }
}

pub fn job_payload(job: JobRecord, runtime: Option<RuntimeJobStatus>) -> InspectPayload {
    InspectPayload::Job { job, runtime }
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

pub fn format_job_inspect(job: &JobRecord, runtime: Option<&RuntimeJobStatus>) -> String {
    let mut output = String::new();
    output.push_str(&format!("Job {}\n", job.id));
    output.push_str(&format!("  name: {}\n", job.name));
    output.push_str(&format!("  status: {}\n", job.status.as_str()));
    output.push_str(&format!("  command: {}\n", job.command));
    output.push_str(&format!("  label: {}\n", job.label));
    output.push_str(&format!("  cwd: {}\n", job.cwd.display()));
    if let Some(repo) = &job.git_repo {
        output.push_str(&format!("  git repo: {}\n", repo.display()));
    }
    if let Some(branch) = &job.git_branch {
        output.push_str(&format!("  git branch: {branch}\n"));
    }
    output.push_str(&format!("  stdout: {}\n", job.stdout_path.display()));
    output.push_str(&format!("  stderr: {}\n", job.stderr_path.display()));
    output.push_str(&format!("  plist: {}\n", job.plist_path.display()));
    output.push_str(&format!("  keep alive: {}\n", job.keep_alive));
    output.push_str(&format!("  restart count: {}\n", job.restart_count));
    output.push_str(&format!("  last exit code: {:?}\n", job.last_exit_code));
    if let Some(runtime) = runtime {
        output.push_str(&format!("  runtime: {}\n", runtime.status.as_str()));
        output.push_str(&format!("  runtime pid: {:?}\n", runtime.pid));
        output.push_str(&format!(
            "  runtime cpu percent: {:?}\n",
            runtime.cpu_percent
        ));
        output.push_str(&format!("  runtime rss kb: {:?}\n", runtime.rss_kb));
        output.push_str(&format!(
            "  runtime restart count: {:?}\n",
            runtime.restart_count
        ));
    }
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
