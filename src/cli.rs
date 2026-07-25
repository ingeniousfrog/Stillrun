use std::io::{self, Write};

use clap::{Args, Parser, Subcommand};

use crate::{
    config::StillrunConfig,
    db::{ExecutionStatus, HistoryFilter, JobRecord, Store},
    execution::{replay_execution, run_foreground, RunRequest},
    history_import::{
        import_selected_shell_history_with_progress, ImportShellSelection, TerminalImportProgress,
    },
    jobs::status::RuntimeJobStatus,
    jobs::{self, BackgroundRunRequest},
    logs,
    output::{execution_summary, job_summary},
    paths::StillrunPaths,
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
    Jobs,
    Logs(LogsArgs),
    Inspect(InspectArgs),
    Status(JobTargetArgs),
    Start(JobTargetArgs),
    Stop(JobTargetArgs),
    Restart(JobTargetArgs),
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
        Commands::History(args) => {
            let records = store.search_history(&HistoryFilter {
                query: args.query,
                cwd: args.cwd,
                repo: args.repo,
                status: args
                    .status
                    .as_deref()
                    .map(ExecutionStatus::parse)
                    .transpose()?,
                started_after_ms: args.since_ms,
                started_before_ms: args.until_ms,
                limit: args.limit,
            })?;
            records
                .iter()
                .for_each(|record| println!("{}", execution_summary(record)));
        }
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
        Commands::Jobs => {
            for job in store.list_jobs()? {
                let (synced_job, runtime) = resolve_and_sync_job_runtime(&store, job).await;
                println!("{} {}", job_summary(&synced_job), runtime_suffix(&runtime));
            }
        }
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
