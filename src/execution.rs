use std::{
    collections::BTreeMap,
    path::PathBuf,
    process::Stdio,
    time::{SystemTime, UNIX_EPOCH},
};

use tokio::process::Command;

use crate::{
    config::StillrunConfig,
    context::CommandContext,
    db::{ExecutionStatus, NewExecution, Store},
    redact::{self, RedactionPolicy},
    Result, StillrunError,
};

#[derive(Debug, Clone)]
pub struct RunRequest {
    pub argv: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcome {
    pub execution_id: i64,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub async fn run_foreground(
    store: &Store,
    config: &StillrunConfig,
    request: RunRequest,
) -> Result<RunOutcome> {
    if request.argv.is_empty() {
        return Err(StillrunError::invalid("run requires a command"));
    }

    let cwd = request.cwd.unwrap_or(std::env::current_dir()?);
    let policy = config.redaction_policy().with_env_values_from_current_env();
    let context = match request.env.clone() {
        Some(env) => CommandContext::from_env(&cwd, env, &policy),
        None => CommandContext::capture_with_policy(&cwd, &policy),
    };
    let started_at_ms = now_ms();
    let spawn_result = build_command(&request.argv, &cwd, request.env.as_ref()).spawn();
    let child = match spawn_result {
        Ok(child) => child,
        Err(err) => {
            let ended_at_ms = now_ms();
            let stderr = redact::redact_inline_secrets(&err.to_string(), &policy);
            let execution_id = store.insert_execution(&NewExecution {
                argv: request.argv,
                context,
                started_at_ms,
                ended_at_ms: Some(ended_at_ms),
                duration_ms: Some(ended_at_ms - started_at_ms),
                exit_code: None,
                status: ExecutionStatus::Failed,
                stdout: String::new(),
                stderr: stderr.clone(),
                pid: None,
                background_job_id: None,
                restart_count: 0,
            })?;
            return Ok(RunOutcome {
                execution_id,
                exit_code: None,
                stdout: String::new(),
                stderr,
            });
        }
    };

    let pid = child.id();
    let output = child.wait_with_output().await?;
    let ended_at_ms = now_ms();
    let exit_code = output.status.code();
    let status = if output.status.success() {
        ExecutionStatus::Success
    } else {
        ExecutionStatus::Failed
    };
    let stdout = truncate_and_redact(&output.stdout, config.max_output_bytes, &policy);
    let stderr = truncate_and_redact(&output.stderr, config.max_output_bytes, &policy);
    let execution_id = store.insert_execution(&NewExecution {
        argv: request.argv,
        context,
        started_at_ms,
        ended_at_ms: Some(ended_at_ms),
        duration_ms: Some(ended_at_ms - started_at_ms),
        exit_code,
        status,
        stdout: stdout.clone(),
        stderr: stderr.clone(),
        pid,
        background_job_id: None,
        restart_count: 0,
    })?;

    Ok(RunOutcome {
        execution_id,
        exit_code,
        stdout,
        stderr,
    })
}

pub async fn replay_execution(
    store: &Store,
    config: &StillrunConfig,
    execution_id: i64,
) -> Result<RunOutcome> {
    let execution = store.get_execution(execution_id)?;
    let env = serde_json::from_str::<BTreeMap<String, String>>(&execution.env_json)
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, value)| value != redact::REDACTED)
        .collect::<BTreeMap<_, _>>();
    run_foreground(
        store,
        config,
        RunRequest {
            argv: execution.argv,
            cwd: Some(execution.cwd),
            env: Some(env),
        },
    )
    .await
}

fn build_command(
    argv: &[String],
    cwd: &PathBuf,
    env: Option<&BTreeMap<String, String>>,
) -> Command {
    let mut command = Command::new(&argv[0]);
    command
        .args(&argv[1..])
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(env) = env {
        command.env_clear();
        command.envs(env);
    }
    command
}

fn truncate_and_redact(bytes: &[u8], max_bytes: usize, policy: &RedactionPolicy) -> String {
    let limit = max_bytes.min(bytes.len());
    let suffix = if bytes.len() > limit {
        "\n[stillrun: output truncated]\n"
    } else {
        ""
    };
    let text = format!("{}{}", String::from_utf8_lossy(&bytes[..limit]), suffix);
    redact::redact_inline_secrets(&text, policy)
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
