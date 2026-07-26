use std::{
    io::{self, IsTerminal, Write},
    process::{Command as StdCommand, Stdio},
    time::Duration,
};

use clap::{Args, Parser, Subcommand};

use crate::{
    config::StillrunConfig,
    db::{ExecutionStatus, HistoryFilter, JobRecord, Store},
    execution::{replay_execution, run_foreground, RunRequest},
    history_import::{
        import_selected_shell_history_with_progress, ImportShellSelection, ShellKind,
        TerminalImportProgress,
    },
    jobs::status::RuntimeJobStatus,
    jobs::{self, BackgroundRunRequest},
    logs,
    output::{execution_summary, format_history_table, job_summary, HistoryDisplayOptions},
    paths::StillrunPaths,
    shell_hook::{self, ShellHookRecord},
    Result,
};

#[derive(Debug, Parser)]
#[command(name = "stillrun")]
#[command(about = "Command lifecycle runtime for macOS jobs.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Run(RunArgs),
    History(HistoryArgs),
    Replay(ReplayArgs),
    Promote(PromoteArgs),
    ImportHistory(ImportHistoryArgs),
    Jobs(JobsArgs),
    Logs(LogsArgs),
    Inspect(InspectArgs),
    Status(JobTargetArgs),
    Start(JobTargetArgs),
    Stop(JobTargetArgs),
    Restart(JobTargetArgs),
    Hook(HookArgs),
}

#[derive(Debug, Args)]
pub struct RunArgs {
    #[arg(short, long)]
    pub background: bool,
    #[arg(short, long)]
    pub name: Option<String>,
    #[arg(long)]
    pub keep_alive: bool,
    #[arg(long)]
    pub cwd: Option<std::path::PathBuf>,
    #[arg(required = true, last = true)]
    pub command: Vec<String>,
}

#[derive(Debug, Args)]
pub struct HistoryArgs {
    #[command(subcommand)]
    pub action: Option<HistoryCommand>,
    #[arg(short, long)]
    pub query: Option<String>,
    #[arg(long)]
    pub cwd: Option<std::path::PathBuf>,
    #[arg(long)]
    pub repo: Option<std::path::PathBuf>,
    #[arg(long)]
    pub status: Option<String>,
    #[arg(long)]
    pub since_ms: Option<i64>,
    #[arg(long)]
    pub until_ms: Option<i64>,
    #[arg(short, long, default_value_t = 25)]
    pub limit: usize,
    #[arg(long)]
    pub full: bool,
    #[arg(long)]
    pub details: bool,
    #[arg(long)]
    pub pager: bool,
    #[arg(long)]
    pub no_pager: bool,
    #[arg(long)]
    pub width: Option<usize>,
}

#[derive(Debug, Subcommand)]
pub enum HistoryCommand {
    Delete(HistoryDeleteArgs),
    Clear(HistoryClearArgs),
    Prune(HistoryPruneArgs),
}

#[derive(Debug, Args)]
pub struct HistoryDeleteArgs {
    pub id: i64,
}

#[derive(Debug, Args)]
pub struct HistoryClearArgs {
    #[arg(long)]
    pub imported: bool,
    #[arg(long)]
    pub source: Option<String>,
    #[arg(long)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct HistoryPruneArgs {
    #[arg(long)]
    pub before_ms: i64,
    #[arg(long)]
    pub source: Option<String>,
    #[arg(long)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct ReplayArgs {
    pub id: i64,
}

#[derive(Debug, Args)]
pub struct PromoteArgs {
    pub id: i64,
    #[arg(short, long)]
    pub name: Option<String>,
    #[arg(long)]
    pub keep_alive: bool,
}

#[derive(Debug, Args)]
pub struct ImportHistoryArgs {
    #[arg(long, value_enum, default_value_t = ImportShellSelection::Auto)]
    pub shell: ImportShellSelection,
    #[arg(long)]
    pub file: Option<std::path::PathBuf>,
}

#[derive(Debug, Args)]
pub struct JobsArgs {
    #[command(subcommand)]
    pub action: Option<JobsCommand>,
}

#[derive(Debug, Subcommand)]
pub enum JobsCommand {
    Delete(JobDeleteArgs),
    Monitor(JobMonitorArgs),
    Events(JobEventsArgs),
    Samples(JobSamplesArgs),
}

#[derive(Debug, Args)]
pub struct JobDeleteArgs {
    pub job: String,
    #[arg(long)]
    pub keep_plist: bool,
}

#[derive(Debug, Args)]
pub struct JobMonitorArgs {
    pub job: String,
    #[arg(long, default_value_t = 5)]
    pub interval_secs: u64,
    #[arg(long)]
    pub once: bool,
    #[arg(long)]
    pub cpu_alert: Option<f32>,
    #[arg(long)]
    pub rss_alert_mb: Option<u64>,
}

#[derive(Debug, Args)]
pub struct JobEventsArgs {
    pub job: String,
    #[arg(short, long, default_value_t = 50)]
    pub limit: usize,
    #[arg(short, long)]
    pub follow: bool,
    #[arg(long, default_value_t = 2)]
    pub interval_secs: u64,
}

#[derive(Debug, Args)]
pub struct JobSamplesArgs {
    pub job: String,
    #[arg(short, long, default_value_t = 50)]
    pub limit: usize,
}

#[derive(Debug, Args)]
pub struct LogsArgs {
    pub job: String,
    #[arg(long)]
    pub stderr: bool,
    #[arg(short, long)]
    pub follow: bool,
    #[arg(short, long, default_value_t = 100)]
    pub lines: usize,
}

#[derive(Debug, Args)]
pub struct InspectArgs {
    pub target: String,
}

#[derive(Debug, Args)]
pub struct JobTargetArgs {
    pub job: String,
}

#[derive(Debug, Args)]
pub struct HookArgs {
    #[command(subcommand)]
    pub action: HookCommand,
}

#[derive(Debug, Subcommand)]
pub enum HookCommand {
    Install(HookInstallArgs),
    Print(HookPrintArgs),
    Record(HookRecordArgs),
}

#[derive(Debug, Args)]
pub struct HookInstallArgs {
    #[arg(long, value_enum, default_value_t = ImportShellSelection::Auto)]
    pub shell: ImportShellSelection,
}

#[derive(Debug, Args)]
pub struct HookPrintArgs {
    #[arg(long, value_enum)]
    pub shell: ShellKind,
}

#[derive(Debug, Args)]
pub struct HookRecordArgs {
    #[arg(long, value_enum)]
    pub shell: ShellKind,
    #[arg(long)]
    pub exit_code: i32,
    #[arg(long)]
    pub cwd: std::path::PathBuf,
    #[arg(long)]
    pub started_ms: Option<i64>,
    #[arg(long)]
    pub source_id: Option<String>,
    #[arg(required = true, last = true)]
    pub command: Vec<String>,
}

pub async fn run() -> Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "stillrun=warn".into()),
        )
        .try_init();

    let cli = Cli::parse();
    let paths = StillrunPaths::discover()?;
    paths.ensure()?;
    let config = StillrunConfig::load(&paths)?;
    let store = Store::open(&paths.db_path)?;
    store.initialize()?;

    match cli.command {
        Commands::Run(args) => {
            if args.background {
                let job = jobs::create_background_job(
                    &store,
                    &paths,
                    &config,
                    BackgroundRunRequest {
                        argv: args.command,
                        name: args.name,
                        cwd: args.cwd,
                        context: None,
                        keep_alive: args.keep_alive,
                    },
                )
                .await?;
                println!("{}", job_summary(&job));
                return Ok(());
            }
            let outcome = run_foreground(
                &store,
                &config,
                RunRequest {
                    argv: args.command,
                    cwd: args.cwd,
                    env: None,
                },
            )
            .await?;
            write_command_output(&outcome.stdout, &outcome.stderr)?;
            exit_with_command_status(outcome.exit_code);
        }
        Commands::History(args) => match args.action {
            Some(HistoryCommand::Delete(delete_args)) => {
                if store.delete_execution(delete_args.id)? {
                    println!("deleted history #{}", delete_args.id);
                } else {
                    return Err(crate::StillrunError::not_found(format!(
                        "execution #{}",
                        delete_args.id
                    )));
                }
            }
            Some(HistoryCommand::Clear(clear_args)) => {
                ensure_bulk_delete_confirmed(clear_args.yes, "history clear")?;
                let deleted = match (clear_args.imported, clear_args.source.as_deref()) {
                    (true, None) => store.clear_imported_history()?,
                    (false, Some(source)) => store.clear_history_source(source)?,
                    (true, Some(_)) => {
                        return Err(crate::StillrunError::invalid(
                            "use either --imported or --source, not both",
                        ))
                    }
                    (false, None) => {
                        return Err(crate::StillrunError::invalid(
                            "history clear requires --imported or --source",
                        ))
                    }
                };
                println!("deleted={deleted}");
            }
            Some(HistoryCommand::Prune(prune_args)) => {
                ensure_bulk_delete_confirmed(prune_args.yes, "history prune")?;
                let deleted = store
                    .prune_history_before(prune_args.before_ms, prune_args.source.as_deref())?;
                println!("deleted={deleted}");
            }
            None => {
                let records = store.search_history(&HistoryFilter {
                    query: args.query.clone(),
                    cwd: args.cwd.clone(),
                    repo: args.repo.clone(),
                    status: args
                        .status
                        .as_deref()
                        .map(ExecutionStatus::parse)
                        .transpose()?,
                    started_after_ms: args.since_ms,
                    started_before_ms: args.until_ms,
                    limit: args.limit,
                })?;
                let output = format_history_table(
                    &records,
                    &HistoryDisplayOptions {
                        width: args.width.or_else(terminal_width).unwrap_or(100),
                        full: args.full,
                        details: args.details,
                    },
                );
                write_history_output(
                    &output,
                    args.pager || (!args.no_pager && should_auto_page(records.len())),
                )?;
            }
        },
        Commands::Replay(args) => {
            let outcome = replay_execution(&store, &config, args.id).await?;
            write_command_output(&outcome.stdout, &outcome.stderr)?;
            exit_with_command_status(outcome.exit_code);
        }
        Commands::Promote(args) => {
            let job = jobs::promote_execution_to_job(
                &store,
                &paths,
                &config,
                args.id,
                args.name,
                args.keep_alive,
            )
            .await?;
            println!("{}", job_summary(&job));
        }
        Commands::ImportHistory(args) => {
            let mut progress = TerminalImportProgress::stderr();
            let summary = import_selected_shell_history_with_progress(
                &store,
                args.shell,
                args.file,
                &mut progress,
            )?;
            println!(
                "imported={} skipped={} scanned={}",
                summary.imported, summary.skipped, summary.scanned
            );
        }
        Commands::Jobs(args) => match args.action {
            Some(JobsCommand::Delete(delete_args)) => {
                let job =
                    jobs::delete_job(&store, &delete_args.job, delete_args.keep_plist).await?;
                println!("deleted job {} plist={}", job.id, job.plist_path.display());
            }
            Some(JobsCommand::Monitor(monitor_args)) => {
                monitor_job(&store, monitor_args).await?;
            }
            Some(JobsCommand::Events(events_args)) => {
                print_job_events(&store, events_args).await?;
            }
            Some(JobsCommand::Samples(samples_args)) => {
                let job = store.find_job(&samples_args.job)?;
                for sample in store.list_job_resource_samples(&job.id, samples_args.limit)? {
                    println!("{}", format_job_sample(&sample));
                }
            }
            None => {
                for job in store.list_jobs()? {
                    let (synced_job, runtime) = resolve_and_sync_job_runtime(&store, job).await;
                    println!("{} {}", job_summary(&synced_job), runtime_suffix(&runtime));
                }
            }
        },
        Commands::Logs(args) => {
            let job = store.find_job(&args.job)?;
            let log_path = if args.stderr {
                job.stderr_path
            } else {
                job.stdout_path
            };
            if args.follow {
                logs::prepare_follow_log_file(&log_path)?;
            }
            let tail = logs::tail_log_file(&log_path, args.lines)?;
            if !tail.is_empty() {
                print!("{tail}");
            }
            if args.follow {
                logs::follow_log_file(&log_path).await?;
            }
        }
        Commands::Inspect(args) => {
            if let Ok(id) = args.target.parse::<i64>() {
                let record = store.get_execution(id)?;
                println!("{}", execution_summary(&record));
                println!("cwd: {}", record.cwd.display());
                if let Some(repo) = record.git_repo {
                    println!("git repo: {}", repo.display());
                }
                if let Some(branch) = record.git_branch {
                    println!("git branch: {branch}");
                }
                println!("exit code: {:?}", record.exit_code);
                println!("duration ms: {:?}", record.duration_ms);
            } else {
                let job = store.find_job(&args.target)?;
                println!("{}", job_summary(&job));
                println!("label: {}", job.label);
                println!("cwd: {}", job.cwd.display());
                println!("stdout: {}", job.stdout_path.display());
                println!("stderr: {}", job.stderr_path.display());
                println!("plist: {}", job.plist_path.display());
                println!("keep alive: {}", job.keep_alive);
                if let Ok(runtime) = jobs::status::resolve_runtime_status(&job).await {
                    let _ = jobs::sync_job_runtime_status(&store, &job, &runtime);
                    println!("runtime: {}", runtime.status.as_str());
                    if let Some(pid) = runtime.pid {
                        println!("pid: {pid}");
                    }
                    if let Some(cpu) = runtime.cpu_percent {
                        println!("cpu percent: {cpu:.1}");
                    }
                    if let Some(rss) = runtime.rss_kb {
                        println!("rss kb: {rss}");
                    }
                    if let Some(code) = runtime.last_exit_code {
                        println!("last exit code: {code}");
                    }
                    if let Some(restarts) = runtime.restart_count {
                        println!("restart count: {restarts}");
                    }
                }
            }
        }
        Commands::Status(args) => {
            let job = store.find_job(&args.job)?;
            let (synced_job, runtime) = resolve_and_sync_job_runtime(&store, job).await;
            println!("{} {}", job_summary(&synced_job), runtime_suffix(&runtime));
            println!("label: {}", synced_job.label);
            println!("stdout: {}", synced_job.stdout_path.display());
            println!("stderr: {}", synced_job.stderr_path.display());
        }
        Commands::Start(args) => {
            let job = jobs::start_job(&store, &args.job).await?;
            println!("{}", job_summary(&job));
        }
        Commands::Stop(args) => {
            let job = jobs::stop_job(&store, &args.job).await?;
            println!("{}", job_summary(&job));
        }
        Commands::Restart(args) => {
            let job = jobs::restart_job(&store, &args.job).await?;
            println!("{}", job_summary(&job));
        }
        Commands::Hook(args) => match args.action {
            HookCommand::Install(install_args) => {
                let installed = shell_hook::install_shell_hook(install_args.shell)?;
                println!(
                    "installed shell hook shell={} path={}",
                    installed.shell.as_str(),
                    installed.path.display()
                );
            }
            HookCommand::Print(print_args) => {
                println!("{}", shell_hook::shell_hook_script(print_args.shell));
            }
            HookCommand::Record(record_args) => {
                let command = record_args.command.join(" ");
                let inserted = shell_hook::record_shell_hook_execution(
                    &store,
                    ShellHookRecord {
                        shell: record_args.shell,
                        command,
                        cwd: record_args.cwd,
                        started_at_ms: record_args.started_ms,
                        exit_code: record_args.exit_code,
                        source_id: record_args.source_id,
                    },
                )?;
                match inserted {
                    Some(id) => println!("recorded history #{id}"),
                    None => println!("recorded history skipped"),
                }
            }
        },
    }
    Ok(())
}

async fn resolve_and_sync_job_runtime(
    store: &Store,
    job: JobRecord,
) -> (JobRecord, RuntimeJobStatus) {
    let runtime = jobs::status::resolve_runtime_status(&job)
        .await
        .unwrap_or_else(|_| RuntimeJobStatus::unknown());
    let synced_job =
        jobs::sync_job_runtime_status(store, &job, &runtime).unwrap_or_else(|_| job.clone());
    (synced_job, runtime)
}

fn write_history_output(output: &str, use_pager: bool) -> Result<()> {
    if output.is_empty() {
        return Ok(());
    }

    if use_pager {
        if let Ok(mut child) = StdCommand::new("less")
            .arg("-R")
            .stdin(Stdio::piped())
            .spawn()
        {
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(output.as_bytes())?;
            }
            let _ = child.wait();
            return Ok(());
        }
    }

    print!("{output}");
    Ok(())
}

fn should_auto_page(record_count: usize) -> bool {
    record_count > 20 && std::io::stdout().is_terminal()
}

fn terminal_width() -> Option<usize> {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|width| *width >= 60)
}

fn ensure_bulk_delete_confirmed(yes: bool, operation: &str) -> Result<()> {
    if yes {
        return Ok(());
    }
    Err(crate::StillrunError::invalid(format!(
        "{operation} is destructive; pass --yes to confirm"
    )))
}

async fn monitor_job(store: &Store, args: JobMonitorArgs) -> Result<()> {
    let interval = Duration::from_secs(args.interval_secs.max(1));
    let rss_alert_kb = args.rss_alert_mb.map(|mb| mb.saturating_mul(1024));
    loop {
        let job = store.find_job(&args.job)?;
        let (synced_job, runtime) = resolve_and_sync_job_runtime(store, job).await;
        let alert_count = jobs::record_resource_alerts(
            store,
            &synced_job,
            &runtime,
            args.cpu_alert,
            rss_alert_kb,
        )?;
        println!(
            "{} {} alerts={alert_count}",
            job_summary(&synced_job),
            runtime_suffix(&runtime)
        );
        if args.once {
            return Ok(());
        }
        tokio::time::sleep(interval).await;
    }
}

async fn print_job_events(store: &Store, args: JobEventsArgs) -> Result<()> {
    let job = store.find_job(&args.job)?;
    let mut after_id = None;
    for event in store.list_job_events(&job.id, args.limit)? {
        after_id = Some(after_id.map_or(event.id, |current: i64| current.max(event.id)));
        println!("{}", format_job_event(&event));
    }
    if !args.follow {
        return Ok(());
    }

    let interval = Duration::from_secs(args.interval_secs.max(1));
    loop {
        tokio::time::sleep(interval).await;
        for event in store.list_job_events_after(&job.id, after_id, args.limit)? {
            after_id = Some(after_id.map_or(event.id, |current: i64| current.max(event.id)));
            println!("{}", format_job_event(&event));
        }
    }
}

fn format_job_sample(sample: &crate::db::JobResourceSample) -> String {
    let pid = sample
        .pid
        .map(|pid| format!(" pid={pid}"))
        .unwrap_or_default();
    let cpu = sample
        .cpu_percent
        .map(|cpu| format!(" cpu={cpu:.1}%"))
        .unwrap_or_default();
    let rss = sample
        .rss_kb
        .map(|rss| format!(" rss={rss}kb"))
        .unwrap_or_default();
    let exit = sample
        .last_exit_code
        .map(|code| format!(" exit={code}"))
        .unwrap_or_default();
    let restarts = sample
        .restart_count
        .map(|count| format!(" restarts={count}"))
        .unwrap_or_default();
    format!(
        "#{} job={} sampled_at={} status={}{}{}{}{}{}",
        sample.id,
        sample.job_id,
        sample.sampled_at_ms,
        sample.status.as_str(),
        pid,
        cpu,
        rss,
        exit,
        restarts
    )
}

fn format_job_event(event: &crate::db::JobEventRecord) -> String {
    let status = event
        .status
        .map(|status| format!(" status={}", status.as_str()))
        .unwrap_or_default();
    let pid = event
        .pid
        .map(|pid| format!(" pid={pid}"))
        .unwrap_or_default();
    let cpu = event
        .cpu_percent
        .map(|cpu| format!(" cpu={cpu:.1}%"))
        .unwrap_or_default();
    let rss = event
        .rss_kb
        .map(|rss| format!(" rss={rss}kb"))
        .unwrap_or_default();
    format!(
        "#{} job={} at={} type={}{}{}{}{} {}",
        event.id,
        event.job_id,
        event.created_at_ms,
        event.event_type,
        status,
        pid,
        cpu,
        rss,
        event.message
    )
}

fn runtime_suffix(runtime: &RuntimeJobStatus) -> String {
    let pid = runtime
        .pid
        .map(|pid| format!(" pid={pid}"))
        .unwrap_or_default();
    let cpu = runtime
        .cpu_percent
        .map(|value| format!(" cpu={value:.1}%"))
        .unwrap_or_default();
    let rss = runtime
        .rss_kb
        .map(|value| format!(" rss={}kb", value))
        .unwrap_or_default();
    let exit = runtime
        .last_exit_code
        .map(|code| format!(" exit={code}"))
        .unwrap_or_default();
    let restarts = runtime
        .restart_count
        .map(|value| format!(" restarts={value}"))
        .unwrap_or_default();
    format!(
        "runtime={}{}{}{}{}{}",
        runtime.status.as_str(),
        pid,
        cpu,
        rss,
        exit,
        restarts
    )
}

fn write_command_output(stdout: &str, stderr: &str) -> Result<()> {
    io::stdout().write_all(stdout.as_bytes())?;
    io::stderr().write_all(stderr.as_bytes())?;
    Ok(())
}

fn exit_with_command_status(exit_code: Option<i32>) {
    if let Some(code) = exit_code {
        if code != 0 {
            std::process::exit(code);
        }
    }
}
