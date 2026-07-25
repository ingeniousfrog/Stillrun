pub mod launchd;
pub mod status;

use std::{collections::BTreeMap, path::PathBuf, process::Stdio};

use tokio::process::Command;

use crate::{
    config::StillrunConfig,
    context::CommandContext,
    db::{
        format_argv, ExecutionStatus, JobRecord, JobRuntimeUpdate, JobStatus, NewExecution, Store,
    },
    execution::now_ms,
    jobs::launchd::LaunchdJobSpec,
    paths::StillrunPaths,
    redact, Result, StillrunError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchdStartAction {
    AlreadyRunning,
    Kickstart,
}

#[derive(Debug, Clone)]
pub struct BackgroundRunRequest {
    pub argv: Vec<String>,
    pub name: Option<String>,
    pub cwd: Option<PathBuf>,
    pub context: Option<CommandContext>,
    pub keep_alive: bool,
}

pub async fn create_background_job(
    store: &Store,
    paths: &StillrunPaths,
    _config: &StillrunConfig,
    request: BackgroundRunRequest,
) -> Result<JobRecord> {
    if request.argv.is_empty() {
        return Err(StillrunError::invalid("background run requires a command"));
    }

    paths.ensure()?;
    std::fs::create_dir_all(&paths.launch_agents_dir)?;
    let context = match request.context {
        Some(context) => context,
        None => {
            let cwd = request.cwd.unwrap_or(std::env::current_dir()?);
            CommandContext::capture(&cwd)
        }
    };
    let cwd = context.cwd.clone();
    let timestamp = now_ms();
    let name = request
        .name
        .unwrap_or_else(|| request.argv[0].replace('/', "-"));
    let id = format!("job-{timestamp}");
    let label = format!("com.stillrun.{}.{}", sanitize_label_part(&name), timestamp);
    let stdout_path = paths.logs_dir.join(format!("{id}.stdout.log"));
    let stderr_path = paths.logs_dir.join(format!("{id}.stderr.log"));
    let plist_path = paths.launch_agents_dir.join(format!("{label}.plist"));
    let persisted_argv = redact::redact_argv(&request.argv);
    let command = redact::redact_inline_secrets(&format_argv(&persisted_argv));
    let spec = LaunchdJobSpec {
        label: label.clone(),
        argv: request.argv.clone(),
        working_directory: cwd.clone(),
        environment: context.restorable_env(),
        stdout_path: stdout_path.clone(),
        stderr_path: stderr_path.clone(),
        keep_alive: request.keep_alive,
    };
    std::fs::write(&plist_path, spec.to_plist_xml())?;

    let created = JobRecord {
        id: id.clone(),
        name,
        label,
        argv: persisted_argv,
        command,
        cwd,
        git_repo: context.git_repo.clone(),
        git_branch: context.git_branch.clone(),
        created_at_ms: timestamp,
        updated_at_ms: timestamp,
        status: JobStatus::Created,
        pid: None,
        restart_count: 0,
        stdout_path,
        stderr_path,
        plist_path,
        keep_alive: request.keep_alive,
        last_exit_code: None,
        last_cpu_percent: None,
        last_rss_kb: None,
    };
    store.upsert_job(&created)?;
    store.insert_execution(&NewExecution {
        argv: created.argv.clone(),
        context,
        started_at_ms: timestamp,
        ended_at_ms: None,
        duration_ms: None,
        exit_code: None,
        status: ExecutionStatus::Background,
        stdout: String::new(),
        stderr: String::new(),
        pid: None,
        background_job_id: Some(id),
        restart_count: 0,
    })?;

    bootstrap_job(&created).await?;
    let running = JobRecord {
        updated_at_ms: now_ms(),
        status: JobStatus::Running,
        ..created
    };
    store.upsert_job(&running)?;
    Ok(running)
}

pub fn background_request_from_execution(
    store: &Store,
    execution_id: i64,
    name: Option<String>,
    keep_alive: bool,
) -> Result<BackgroundRunRequest> {
    let execution = store.get_execution(execution_id)?;
    let env = serde_json::from_str::<BTreeMap<String, String>>(&execution.env_json)
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, value)| value != redact::REDACTED)
        .collect();
    let context = CommandContext {
        cwd: execution.cwd.clone(),
        git_repo: execution.git_repo.clone(),
        git_branch: execution.git_branch.clone(),
        env,
    };
    Ok(BackgroundRunRequest {
        argv: execution.argv,
        name,
        cwd: None,
        context: Some(context),
        keep_alive,
    })
}

pub async fn promote_execution_to_job(
    store: &Store,
    paths: &StillrunPaths,
    config: &StillrunConfig,
    execution_id: i64,
    name: Option<String>,
    keep_alive: bool,
) -> Result<JobRecord> {
    let request = background_request_from_execution(store, execution_id, name, keep_alive)?;
    create_background_job(store, paths, config, request).await
}

pub async fn stop_job(store: &Store, target: &str) -> Result<JobRecord> {
    let job = store.find_job(target)?;
    bootout_job(&job).await?;
    let stopped = JobRecord {
        updated_at_ms: now_ms(),
        status: JobStatus::Stopped,
        pid: None,
        ..job
    };
    store.upsert_job(&stopped)?;
    Ok(stopped)
}

pub async fn start_job(store: &Store, target: &str) -> Result<JobRecord> {
    let job = store.find_job(target)?;
    if let Some(runtime) = status::resolve_loaded_runtime_status(&job).await? {
        match start_action_for_loaded_runtime_status(&runtime) {
            LaunchdStartAction::AlreadyRunning => {
                return sync_job_runtime_status(store, &job, &runtime);
            }
            LaunchdStartAction::Kickstart => {
                kickstart_job(&job).await?;
            }
        }
    } else {
        bootstrap_job(&job).await?;
    }
    let running = JobRecord {
        updated_at_ms: now_ms(),
        status: JobStatus::Running,
        pid: None,
        ..job
    };
    store.upsert_job(&running)?;
    Ok(running)
}

pub fn start_action_for_loaded_runtime_status(
    runtime: &status::RuntimeJobStatus,
) -> LaunchdStartAction {
    if runtime.status == JobStatus::Running {
        LaunchdStartAction::AlreadyRunning
    } else {
        LaunchdStartAction::Kickstart
    }
}

pub async fn restart_job(store: &Store, target: &str) -> Result<JobRecord> {
    let job = store.find_job(target)?;
    let _ = bootout_job(&job).await;
    bootstrap_job(&job).await?;
    let running = JobRecord {
        updated_at_ms: now_ms(),
        status: JobStatus::Running,
        restart_count: job.restart_count + 1,
        ..job
    };
    store.upsert_job(&running)?;
    Ok(running)
}

pub fn sync_job_runtime_status(
    store: &Store,
    job: &JobRecord,
    runtime: &status::RuntimeJobStatus,
) -> Result<JobRecord> {
    store.update_job_runtime(
        &job.id,
        &JobRuntimeUpdate {
            status: runtime.status,
            pid: runtime.pid,
            last_exit_code: runtime.last_exit_code,
            cpu_percent: runtime.cpu_percent,
            rss_kb: runtime.rss_kb,
            restart_count: runtime.restart_count,
            updated_at_ms: now_ms(),
        },
    )
}

async fn bootstrap_job(job: &JobRecord) -> Result<()> {
    require_macos_launchd()?;
    let domain = launchd_domain().await?;
    let output = Command::new("launchctl")
        .args(["bootstrap", &domain])
        .arg(&job.plist_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;
    if output.status.success() {
        return Ok(());
    }
    Err(StillrunError::invalid(format!(
        "launchctl bootstrap failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}

async fn kickstart_job(job: &JobRecord) -> Result<()> {
    require_macos_launchd()?;
    let domain = launchd_domain().await?;
    let target = format!("{domain}/{}", job.label);
    let output = Command::new("launchctl")
        .args(["kickstart", &target])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;
    if output.status.success() {
        return Ok(());
    }
    Err(StillrunError::invalid(format!(
        "launchctl kickstart failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}

async fn bootout_job(job: &JobRecord) -> Result<()> {
    require_macos_launchd()?;
    let domain = launchd_domain().await?;
    let output = Command::new("launchctl")
        .args(["bootout", &domain])
        .arg(&job.plist_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if is_already_unloaded_bootout_error(&stderr) {
        return Ok(());
    }
    Err(StillrunError::invalid(format!(
        "launchctl bootout failed: {}",
        stderr.trim()
    )))
}

pub fn is_already_unloaded_bootout_error(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    lower.contains("no such process")
        || lower.contains("could not find service")
        || lower.contains("service not found")
}

fn require_macos_launchd() -> Result<()> {
    if cfg!(target_os = "macos") {
        Ok(())
    } else {
        Err(StillrunError::unsupported(
            "background jobs use macOS launchd in the MVP",
        ))
    }
}

pub(crate) async fn launchd_domain() -> Result<String> {
    let output = Command::new("id")
        .arg("-u")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;
    if !output.status.success() {
        return Err(StillrunError::invalid(format!(
            "failed to resolve user id: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(format!(
        "gui/{}",
        String::from_utf8_lossy(&output.stdout).trim()
    ))
}

fn sanitize_label_part(input: &str) -> String {
    let sanitized = input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "job".to_string()
    } else {
        sanitized
    }
}
