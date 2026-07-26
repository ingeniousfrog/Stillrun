use std::process::Stdio;

use serde::Serialize;
use tokio::process::Command;

use crate::{
    db::{JobRecord, JobStatus},
    jobs::launchd_domain,
    Result, StillrunError,
};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RuntimeJobStatus {
    pub status: JobStatus,
    pub pid: Option<u32>,
    pub cpu_percent: Option<f32>,
    pub rss_kb: Option<u64>,
    pub last_exit_code: Option<i32>,
    pub restart_count: Option<i64>,
}

impl RuntimeJobStatus {
    pub fn unknown() -> Self {
        Self {
            status: JobStatus::Unknown,
            pid: None,
            cpu_percent: None,
            rss_kb: None,
            last_exit_code: None,
            restart_count: None,
        }
    }
}

pub async fn resolve_runtime_status(job: &JobRecord) -> Result<RuntimeJobStatus> {
    Ok(resolve_loaded_runtime_status(job)
        .await?
        .unwrap_or(RuntimeJobStatus {
            status: JobStatus::Stopped,
            pid: None,
            cpu_percent: None,
            rss_kb: None,
            last_exit_code: None,
            restart_count: None,
        }))
}

pub async fn resolve_loaded_runtime_status(job: &JobRecord) -> Result<Option<RuntimeJobStatus>> {
    if !cfg!(target_os = "macos") {
        return Err(StillrunError::unsupported(
            "runtime status uses macOS launchd in the MVP",
        ));
    }
    let domain = launchd_domain().await?;
    let output = Command::new("launchctl")
        .args(["print", &format!("{domain}/{}", job.label)])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;
    if !output.status.success() {
        return Ok(None);
    }
    let launchd_text = String::from_utf8_lossy(&output.stdout);
    let mut launchd_status = parse_launchd_print(&launchd_text);
    if let Some(pid) = launchd_status.pid {
        if let Ok((cpu_percent, rss_kb)) = sample_process_resources(pid).await {
            launchd_status.cpu_percent = cpu_percent;
            launchd_status.rss_kb = rss_kb;
        }
    }
    Ok(Some(launchd_status))
}

pub fn parse_launchd_print(text: &str) -> RuntimeJobStatus {
    let pid = text.lines().find_map(parse_pid_line);
    let last_exit_code = text.lines().find_map(parse_last_exit_code_line);
    let restart_count = text
        .lines()
        .find_map(parse_runs_line)
        .map(|runs| runs.saturating_sub(1));
    let status = if pid.is_some() {
        JobStatus::Running
    } else if last_exit_code.is_some_and(|code| code != 0) {
        JobStatus::Failed
    } else if text.contains("state = running") {
        JobStatus::Running
    } else if text.contains("last exit code = 0") {
        JobStatus::Stopped
    } else {
        JobStatus::Unknown
    };
    RuntimeJobStatus {
        status,
        pid,
        cpu_percent: None,
        rss_kb: None,
        last_exit_code,
        restart_count,
    }
}

pub fn parse_ps_output(text: &str) -> Option<(f32, u64)> {
    text.lines().find_map(|line| {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.len() < 2 {
            return None;
        }
        let cpu = fields[0].parse::<f32>().ok()?;
        let rss = fields[1].parse::<u64>().ok()?;
        Some((cpu, rss))
    })
}

async fn sample_process_resources(pid: u32) -> Result<(Option<f32>, Option<u64>)> {
    let output = Command::new("ps")
        .args(["-o", "%cpu=", "-o", "rss=", "-p", &pid.to_string()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;
    let (cpu_percent, rss_kb) = parse_ps_output(&String::from_utf8_lossy(&output.stdout))
        .map(|(cpu, rss)| (Some(cpu), Some(rss)))
        .unwrap_or((None, None));
    Ok((cpu_percent, rss_kb))
}

fn parse_pid_line(line: &str) -> Option<u32> {
    let trimmed = line.trim();
    trimmed
        .strip_prefix("pid = ")
        .and_then(|value| value.trim().parse::<u32>().ok())
}

fn parse_last_exit_code_line(line: &str) -> Option<i32> {
    let trimmed = line.trim();
    trimmed
        .strip_prefix("last exit code = ")
        .and_then(|value| value.trim().parse::<i32>().ok())
}

fn parse_runs_line(line: &str) -> Option<i64> {
    let trimmed = line.trim();
    trimmed
        .strip_prefix("runs = ")
        .and_then(|value| value.trim().parse::<i64>().ok())
}
