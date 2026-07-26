use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{
    db::{JobEventRecord, JobRecord, JobResourceSample, JobStatus, Store},
    jobs::status::RuntimeJobStatus,
    logs, Result,
};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LogPreview {
    pub path: PathBuf,
    pub available: bool,
    pub lines: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JobDashboard {
    pub job: JobRecord,
    pub runtime: RuntimeJobStatus,
    pub last_sample: Option<JobResourceSample>,
    pub recent_events: Vec<JobEventRecord>,
    pub stdout: LogPreview,
    pub stderr: LogPreview,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JobListEntry {
    pub job: JobRecord,
    pub runtime: RuntimeJobStatus,
    pub last_sample: Option<JobResourceSample>,
}

pub fn build_job_dashboard(
    store: &Store,
    job: JobRecord,
    runtime: RuntimeJobStatus,
) -> Result<JobDashboard> {
    Ok(JobDashboard {
        last_sample: store
            .list_job_resource_samples(&job.id, 1)?
            .into_iter()
            .next(),
        recent_events: store.list_job_events(&job.id, 8)?,
        stdout: read_log_preview(&job.stdout_path, 5),
        stderr: read_log_preview(&job.stderr_path, 5),
        job,
        runtime,
    })
}

pub fn read_log_preview(path: &Path, lines: usize) -> LogPreview {
    match logs::tail_log_file(path, lines) {
        Ok(tail) => LogPreview {
            path: path.to_path_buf(),
            available: true,
            lines: tail.lines().map(str::to_string).collect(),
            error: None,
        },
        Err(err) => LogPreview {
            path: path.to_path_buf(),
            available: false,
            lines: Vec::new(),
            error: Some(err.to_string()),
        },
    }
}

pub fn format_job_dashboard(dashboard: &JobDashboard) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "Job {} ({})\n",
        dashboard.job.name, dashboard.job.id
    ));
    output.push_str(&format!("Status: {}\n", dashboard.runtime.status.as_str()));
    output.push_str(&format!("Command: {}\n", dashboard.job.command));
    output.push_str(&format!("Label: {}\n", dashboard.job.label));
    output.push_str(&format!("Cwd: {}\n", dashboard.job.cwd.display()));
    output.push_str(&format!("PID: {}\n", option_display(dashboard.runtime.pid)));
    output.push_str(&format!(
        "CPU: {} RSS: {}\n",
        dashboard
            .runtime
            .cpu_percent
            .map(|cpu| format!("{cpu:.1}%"))
            .unwrap_or_else(|| "-".into()),
        dashboard
            .runtime
            .rss_kb
            .map(|rss| format!("{rss}kb"))
            .unwrap_or_else(|| "-".into())
    ));
    output.push_str(&format!(
        "Restart count: {}\n",
        dashboard
            .runtime
            .restart_count
            .unwrap_or(dashboard.job.restart_count)
    ));
    output.push_str(&format!("Last exit: {}\n", last_exit_summary(dashboard)));
    output.push_str(&format!(
        "Last sample: {}\n",
        last_sample_summary(dashboard)
    ));
    output.push_str(&format!(
        "Stdout: {}\n",
        dashboard.job.stdout_path.display()
    ));
    output.push_str(&format!(
        "Stderr: {}\n",
        dashboard.job.stderr_path.display()
    ));
    append_log_section(&mut output, "Recent stdout", &dashboard.stdout);
    append_log_section(&mut output, "Recent stderr", &dashboard.stderr);
    append_event_section(&mut output, &dashboard.recent_events);
    output
}

pub fn format_job_list(entries: &[JobListEntry]) -> String {
    if entries.is_empty() {
        return "No jobs found.\n".into();
    }

    let mut output = String::new();
    for group in [
        JobStatus::Running,
        JobStatus::Failed,
        JobStatus::Stopped,
        JobStatus::Created,
        JobStatus::Unknown,
    ] {
        let group_entries = entries
            .iter()
            .filter(|entry| entry.runtime.status == group)
            .collect::<Vec<_>>();
        if group_entries.is_empty() {
            continue;
        }
        output.push_str(&format!(
            "{} ({})\n",
            group.as_str().to_ascii_uppercase(),
            group_entries.len()
        ));
        for entry in group_entries {
            output.push_str(&format!("  {}\n", format_job_list_entry(entry)));
        }
    }
    output
}

pub fn format_job_timeline(job: &JobRecord, events: &[JobEventRecord]) -> String {
    let mut output = String::new();
    output.push_str(&format!("Timeline for {} ({})\n", job.name, job.id));
    if events.is_empty() {
        output.push_str("  No events recorded.\n");
        return output;
    }

    let mut ordered = events.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|event| (event.created_at_ms, event.id));
    for event in ordered {
        output.push_str(&format!("  {}\n", format_job_timeline_event(event)));
    }
    output
}

pub fn format_job_timeline_event(event: &JobEventRecord) -> String {
    let mut parts = vec![
        format!("[{}]", event.created_at_ms),
        event.event_type.clone(),
    ];
    if let Some(status) = event.status {
        parts.push(format!("status={}", status.as_str()));
    }
    if should_show_pid(event) {
        if let Some(pid) = event.pid {
            parts.push(format!("pid={pid}"));
        }
    }
    if should_show_exit(event) {
        if let Some(exit) = event.last_exit_code {
            parts.push(format!("exit={exit}"));
        }
    }
    if should_show_resources(event) {
        if let Some(cpu) = event.cpu_percent {
            parts.push(format!("cpu={cpu:.1}%"));
        }
        if let Some(rss) = event.rss_kb {
            parts.push(format!("rss={rss}kb"));
        }
    }
    parts.push(event.message.clone());
    parts.join(" ")
}

pub fn format_sample_summary(sample: &JobResourceSample) -> String {
    let mut parts = vec![
        sample.sampled_at_ms.to_string(),
        format!("status={}", sample.status.as_str()),
    ];
    if let Some(pid) = sample.pid {
        parts.push(format!("pid={pid}"));
    }
    if let Some(cpu) = sample.cpu_percent {
        parts.push(format!("cpu={cpu:.1}%"));
    }
    if let Some(rss) = sample.rss_kb {
        parts.push(format!("rss={rss}kb"));
    }
    if let Some(exit) = sample.last_exit_code {
        parts.push(format!("exit={exit}"));
    }
    if let Some(restarts) = sample.restart_count {
        parts.push(format!("restarts={restarts}"));
    }
    parts.join(" ")
}

fn format_job_list_entry(entry: &JobListEntry) -> String {
    let prefix = if entry.runtime.status == JobStatus::Failed {
        "!"
    } else {
        "-"
    };
    let runtime = runtime_summary(&entry.runtime);
    let last_sample = entry
        .last_sample
        .as_ref()
        .map(|sample| format!(" last_sample={}", sample.sampled_at_ms))
        .unwrap_or_default();
    format!(
        "{} {} ({}) {}{} command={}",
        prefix, entry.job.name, entry.job.id, runtime, last_sample, entry.job.command
    )
}

fn runtime_summary(runtime: &RuntimeJobStatus) -> String {
    let mut parts = vec![format!("status={}", runtime.status.as_str())];
    if let Some(pid) = runtime.pid {
        parts.push(format!("pid={pid}"));
    }
    if let Some(cpu) = runtime.cpu_percent {
        parts.push(format!("cpu={cpu:.1}%"));
    }
    if let Some(rss) = runtime.rss_kb {
        parts.push(format!("rss={rss}kb"));
    }
    if let Some(exit) = runtime.last_exit_code {
        parts.push(format!("exit={exit}"));
    }
    if let Some(restarts) = runtime.restart_count {
        parts.push(format!("restarts={restarts}"));
    }
    parts.join(" ")
}

fn last_exit_summary(dashboard: &JobDashboard) -> String {
    let exit = dashboard
        .runtime
        .last_exit_code
        .or(dashboard.job.last_exit_code)
        .map(|code| format!("code={code}"))
        .unwrap_or_else(|| "-".into());
    let reason = dashboard
        .recent_events
        .iter()
        .find(|event| event.event_type == "exit")
        .map(|event| event.message.as_str());
    match reason {
        Some(reason) if exit == "-" => reason.to_string(),
        Some(reason) => format!("{exit} ({reason})"),
        None => exit,
    }
}

fn last_sample_summary(dashboard: &JobDashboard) -> String {
    dashboard
        .last_sample
        .as_ref()
        .map(format_sample_summary)
        .unwrap_or_else(|| "-".into())
}

fn append_log_section(output: &mut String, title: &str, preview: &LogPreview) {
    output.push_str(&format!("{title}:\n"));
    if !preview.available {
        output.push_str(&format!(
            "  unavailable: {}\n",
            preview.error.as_deref().unwrap_or("log not available")
        ));
        return;
    }
    if preview.lines.is_empty() {
        output.push_str("  -\n");
        return;
    }
    for line in &preview.lines {
        output.push_str(&format!("  {line}\n"));
    }
}

fn append_event_section(output: &mut String, events: &[JobEventRecord]) {
    output.push_str("Recent events:\n");
    if events.is_empty() {
        output.push_str("  -\n");
        return;
    }
    for event in events.iter().take(5) {
        output.push_str(&format!("  {}\n", format_job_timeline_event(event)));
    }
}

fn option_display<T: std::fmt::Display>(value: Option<T>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".into())
}

fn should_show_pid(event: &JobEventRecord) -> bool {
    matches!(event.event_type.as_str(), "pid" | "exit")
}

fn should_show_exit(event: &JobEventRecord) -> bool {
    event.event_type == "exit"
}

fn should_show_resources(event: &JobEventRecord) -> bool {
    event.event_type == "exit" || event.event_type.starts_with("alert.")
}
