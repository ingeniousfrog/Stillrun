pub mod launchd;
pub mod status;

use std::{collections::BTreeMap, path::PathBuf, process::Stdio};

use tokio::process::Command;

use crate::{
    config::StillrunConfig,
    context::CommandContext,
    db::{
        format_argv, ExecutionStatus, JobRecord, JobRuntimeUpdate, JobStatus, NewExecution,
        NewJobEvent, NewJobResourceSample, Store,
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

#[derive(Debug, Clone, PartialEq)]
pub struct JobDeleteReport {
    pub job: JobRecord,
    pub launchd_warning: Option<String>,
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
    record_job_lifecycle_event(store, &created, "created", "job created")?;
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
    record_job_lifecycle_event(store, &running, "started", "job started")?;
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
    bootout_job(&job)
        .await
        .map_err(|err| launchd_operation_error("stop", &job, err))?;
    let stopped = JobRecord {
        updated_at_ms: now_ms(),
        status: JobStatus::Stopped,
        pid: None,
        ..job
    };
    store.upsert_job(&stopped)?;
    record_job_lifecycle_event(store, &stopped, "stopped", "job stopped")?;
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
                kickstart_job(&job)
                    .await
                    .map_err(|err| launchd_operation_error("start", &job, err))?;
            }
        }
    } else {
        bootstrap_job(&job)
            .await
            .map_err(|err| launchd_operation_error("start", &job, err))?;
    }
    let running = JobRecord {
        updated_at_ms: now_ms(),
        status: JobStatus::Running,
        pid: None,
        ..job
    };
    store.upsert_job(&running)?;
    record_job_lifecycle_event(store, &running, "started", "job started")?;
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
    bootout_job(&job)
        .await
        .map_err(|err| launchd_operation_error("stop before restart", &job, err))?;
    bootstrap_job(&job)
        .await
        .map_err(|err| launchd_operation_error("start after restart", &job, err))?;
    let running = JobRecord {
        updated_at_ms: now_ms(),
        status: JobStatus::Running,
        restart_count: job.restart_count + 1,
        ..job
    };
    store.upsert_job(&running)?;
    record_job_lifecycle_event(store, &running, "restarted", "job restarted")?;
    Ok(running)
}

pub async fn delete_job(store: &Store, target: &str, keep_plist: bool) -> Result<JobRecord> {
    Ok(delete_job_with_report(store, target, keep_plist).await?.job)
}

pub async fn delete_job_with_report(
    store: &Store,
    target: &str,
    keep_plist: bool,
) -> Result<JobDeleteReport> {
    let job = store.find_job(target)?;
    let mut launchd_warning = None;
    if cfg!(target_os = "macos") {
        if let Err(err) = bootout_job(&job).await {
            let warning = launchd_warning_for_delete(&job, err);
            tracing::warn!(job = %job.id, warning = %warning, "failed to unload launchd job before delete");
            launchd_warning = Some(warning);
        }
    }
    if !keep_plist && job.plist_path.exists() {
        std::fs::remove_file(&job.plist_path)?;
    }
    store.delete_job_record(&job.id)?;
    Ok(JobDeleteReport {
        job,
        launchd_warning,
    })
}

pub fn sync_job_runtime_status(
    store: &Store,
    job: &JobRecord,
    runtime: &status::RuntimeJobStatus,
) -> Result<JobRecord> {
    let observed_at_ms = now_ms();
    store.record_job_resource_sample(&NewJobResourceSample {
        job_id: job.id.clone(),
        sampled_at_ms: observed_at_ms,
        status: runtime.status,
        pid: runtime.pid,
        last_exit_code: runtime.last_exit_code,
        cpu_percent: runtime.cpu_percent,
        rss_kb: runtime.rss_kb,
        restart_count: runtime.restart_count,
    })?;
    record_runtime_change_events(store, job, runtime, observed_at_ms)?;
    store.update_job_runtime(
        &job.id,
        &JobRuntimeUpdate {
            status: runtime.status,
            pid: runtime.pid,
            last_exit_code: runtime.last_exit_code,
            cpu_percent: runtime.cpu_percent,
            rss_kb: runtime.rss_kb,
            restart_count: runtime.restart_count,
            updated_at_ms: observed_at_ms,
        },
    )
}

pub fn record_resource_alerts(
    store: &Store,
    job: &JobRecord,
    runtime: &status::RuntimeJobStatus,
    cpu_alert_percent: Option<f32>,
    rss_alert_kb: Option<u64>,
) -> Result<usize> {
    let created_at_ms = now_ms();
    let mut recorded = 0;
    if let (Some(cpu), Some(threshold)) = (runtime.cpu_percent, cpu_alert_percent) {
        if cpu >= threshold {
            store.record_job_event(&NewJobEvent {
                job_id: job.id.clone(),
                created_at_ms,
                event_type: "alert.cpu".into(),
                message: format!("cpu {cpu:.1}% >= {threshold:.1}%"),
                status: Some(runtime.status),
                pid: runtime.pid,
                last_exit_code: runtime.last_exit_code,
                cpu_percent: runtime.cpu_percent,
                rss_kb: runtime.rss_kb,
            })?;
            recorded += 1;
        }
    }
    if let (Some(rss), Some(threshold)) = (runtime.rss_kb, rss_alert_kb) {
        if rss >= threshold {
            store.record_job_event(&NewJobEvent {
                job_id: job.id.clone(),
                created_at_ms,
                event_type: "alert.rss".into(),
                message: format!("rss {rss}kb >= {threshold}kb"),
                status: Some(runtime.status),
                pid: runtime.pid,
                last_exit_code: runtime.last_exit_code,
                cpu_percent: runtime.cpu_percent,
                rss_kb: runtime.rss_kb,
            })?;
            recorded += 1;
        }
    }
    Ok(recorded)
}

fn record_job_lifecycle_event(
    store: &Store,
    job: &JobRecord,
    event_type: &str,
    message: &str,
) -> Result<()> {
    store.record_job_event(&NewJobEvent {
        job_id: job.id.clone(),
        created_at_ms: job.updated_at_ms,
        event_type: event_type.into(),
        message: message.into(),
        status: Some(job.status),
        pid: job.pid,
        last_exit_code: job.last_exit_code,
        cpu_percent: job.last_cpu_percent,
        rss_kb: job.last_rss_kb,
    })?;
    Ok(())
}

fn record_runtime_change_events(
    store: &Store,
    job: &JobRecord,
    runtime: &status::RuntimeJobStatus,
    observed_at_ms: i64,
) -> Result<()> {
    if job.status != runtime.status {
        store.record_job_event(&runtime_event(
            job,
            runtime,
            observed_at_ms,
            "status",
            format!(
                "status {} -> {}",
                job.status.as_str(),
                runtime.status.as_str()
            ),
        ))?;
    }
    if job.pid != runtime.pid {
        store.record_job_event(&runtime_event(
            job,
            runtime,
            observed_at_ms,
            "pid",
            format!(
                "pid {} -> {}",
                optional_value(job.pid),
                optional_value(runtime.pid)
            ),
        ))?;
    }
    if runtime.last_exit_code.is_some() && job.last_exit_code != runtime.last_exit_code {
        let message = runtime
            .last_exit_code
            .map(|code| format!("exit code {code}"))
            .unwrap_or_else(|| "exit code observed".into());
        store.record_job_event(&runtime_event(
            job,
            runtime,
            observed_at_ms,
            "exit",
            message,
        ))?;
    }
    if runtime
        .restart_count
        .is_some_and(|restart_count| restart_count > job.restart_count)
    {
        store.record_job_event(&runtime_event(
            job,
            runtime,
            observed_at_ms,
            "restart",
            format!(
                "restart count {} -> {}",
                job.restart_count,
                runtime.restart_count.unwrap_or(job.restart_count)
            ),
        ))?;
    }
    Ok(())
}

fn runtime_event(
    job: &JobRecord,
    runtime: &status::RuntimeJobStatus,
    created_at_ms: i64,
    event_type: &str,
    message: String,
) -> NewJobEvent {
    NewJobEvent {
        job_id: job.id.clone(),
        created_at_ms,
        event_type: event_type.into(),
        message,
        status: Some(runtime.status),
        pid: runtime.pid,
        last_exit_code: runtime.last_exit_code,
        cpu_percent: runtime.cpu_percent,
        rss_kb: runtime.rss_kb,
    }
}

async fn bootstrap_job(job: &JobRecord) -> Result<()> {
    require_macos_launchd()?;
    ensure_launchd_plist_exists(job, "start")?;
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
    ensure_launchd_plist_exists(job, "stop")?;
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

pub fn format_launchd_operation_error(
    operation: &str,
    job: &JobRecord,
    err: StillrunError,
) -> String {
    format!(
        "could not {operation} job '{}' ({}) via launchd: {err}. plist={}. Try `stillrun status {}` to inspect the current state; if the plist is stale, use `stillrun jobs delete {}` to remove the local record.",
        job.name,
        job.id,
        job.plist_path.display(),
        job.name,
        job.name
    )
}

fn launchd_operation_error(operation: &str, job: &JobRecord, err: StillrunError) -> StillrunError {
    StillrunError::invalid(format_launchd_operation_error(operation, job, err))
}

fn launchd_warning_for_delete(job: &JobRecord, err: StillrunError) -> String {
    format!(
        "could not unload job '{}' ({}) from launchd before deleting the local record: {err}. plist={}",
        job.name,
        job.id,
        job.plist_path.display()
    )
}

fn ensure_launchd_plist_exists(job: &JobRecord, operation: &str) -> Result<()> {
    if job.plist_path.exists() {
        return Ok(());
    }
    Err(StillrunError::not_found(format!(
        "launchd plist for job '{}' ({}) at {}; cannot {operation}. Use `stillrun jobs delete {}` to remove the stale local record.",
        job.name,
        job.id,
        job.plist_path.display(),
        job.name
    )))
}

fn optional_value<T: std::fmt::Display>(value: Option<T>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".into())
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
