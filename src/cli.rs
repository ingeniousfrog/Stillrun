use std::{
    io::{self, IsTerminal, Write},
    process::{Command as StdCommand, Stdio},
};

use clap::{Args, Parser, Subcommand, ValueEnum};

pub use crate::{
    completion::{CompletionArgs, CompletionCandidateKind, CompletionCommand},
    config_cli::{ConfigArgs, ConfigCommand, ConfigRedactCommand},
    job_cli::{
        JobDeleteArgs, JobEventsArgs, JobMonitorArgs, JobSamplesArgs, JobsArgs, JobsCommand,
    },
};

use crate::{
    config::StillrunConfig,
    db::{ExecutionStatus, HistoryFilter, HistorySortOrder, Store},
    execution::{replay_execution, run_foreground, RunRequest},
    history_import::{
        format_import_preview, import_selected_shell_history_with_progress,
        preview_selected_shell_history, ImportPreview, ImportShellSelection, ShellKind,
        TerminalImportProgress,
    },
    inspect,
    jobs::{self, BackgroundRunRequest},
    logs,
    output::{format_history_table, job_summary, HistoryDisplayOptions},
    paths::StillrunPaths,
    shell_hook::{self, ShellHookRecord},
    Result,
};

const ROOT_HELP: &str = r#"Capabilities
  Run and record:       stillrun run -- npm run dev
  Search history:       stillrun history --query "npm" --sort oldest
  Import shell history: stillrun import-history --shell auto --preview
  Replay safely:        stillrun replay 12 --preview
  Promote to Job:       stillrun promote 12 --name dev-server
  Manage Jobs:          stillrun jobs | stillrun status dev-server | stillrun logs dev-server
  Inspect as JSON:      stillrun inspect 12 --json
  Manage config:        stillrun config show | stillrun config set max-output-bytes 2097152
  Shell completion:     stillrun completion zsh > ~/.stillrun-completion.zsh

Examples
  stillrun run -- zsh -lc "npm run dev 2>&1 | tee dev.log"
  stillrun history --status imported --query "cargo"
  stillrun run --background --name api -- cargo run
  stillrun jobs monitor api --once --cpu-alert 90 --rss-alert-mb 1024
  stillrun config redact add session_token

Use `stillrun <command> -h` for command-specific flags."#;

#[derive(Debug, Parser)]
#[command(name = "stillrun")]
#[command(about = "Command lifecycle runtime for macOS jobs.")]
#[command(after_help = ROOT_HELP)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(about = "Run and record a foreground command.")]
    Run(RunArgs),
    #[command(about = "Search, inspect, and maintain execution history.")]
    History(HistoryArgs),
    #[command(about = "Replay a recorded execution.")]
    Replay(ReplayArgs),
    #[command(about = "Promote a recorded execution into a launchd Job.")]
    Promote(PromoteArgs),
    #[command(about = "Preview and import local shell history.")]
    ImportHistory(ImportHistoryArgs),
    #[command(about = "List and manage background Jobs.")]
    Jobs(JobsArgs),
    #[command(about = "Read or follow Job stdout/stderr logs.")]
    Logs(LogsArgs),
    #[command(about = "Inspect an execution or Job, optionally as JSON.")]
    Inspect(InspectArgs),
    #[command(about = "Show one Job's runtime status.")]
    Status(JobTargetArgs),
    #[command(about = "Start a stored Job.")]
    Start(JobTargetArgs),
    #[command(about = "Stop a stored Job.")]
    Stop(JobTargetArgs),
    #[command(about = "Restart a stored Job.")]
    Restart(JobTargetArgs),
    #[command(about = "Install, print, or record shell hook events.")]
    Hook(HookArgs),
    #[command(about = "Show and update local Stillrun config.")]
    Config(ConfigArgs),
    #[command(about = "Print shell completion scripts or dynamic candidates.")]
    Completion(CompletionArgs),
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
    #[arg(long, value_enum, default_value_t = HistorySort::Newest)]
    pub sort: HistorySort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum HistorySort {
    Newest,
    Oldest,
}

impl From<HistorySort> for HistorySortOrder {
    fn from(value: HistorySort) -> Self {
        match value {
            HistorySort::Newest => Self::NewestFirst,
            HistorySort::Oldest => Self::OldestFirst,
        }
    }
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
    #[arg(long)]
    pub preview: bool,
    #[arg(long)]
    pub yes: bool,
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
    #[arg(long)]
    pub preview: bool,
    #[arg(long)]
    pub yes: bool,
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
    #[arg(long)]
    pub json: bool,
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
                    sort: args.sort.into(),
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
            let record = store.get_execution(args.id)?;
            if args.preview {
                print!("{}", inspect::format_replay_preview(&record));
                return Ok(());
            }
            if requires_replay_confirmation(&record) {
                ensure_replay_confirmed(args.yes, &record)?;
            }
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
            let preview = preview_selected_shell_history(&store, args.shell, args.file.clone())?;
            print!("{}", format_import_preview(&preview));
            if args.preview {
                return Ok(());
            }
            ensure_import_confirmed(args.yes, &preview)?;
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
        Commands::Jobs(args) => {
            crate::job_cli::handle_jobs_command(&store, args).await?;
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
                if args.json {
                    println!(
                        "{}",
                        serde_json::to_string(&inspect::execution_payload(record))?
                    );
                } else {
                    print!("{}", inspect::format_execution_inspect(&record));
                }
            } else {
                let mut job = store.find_job(&args.target)?;
                let runtime = match jobs::status::resolve_runtime_status(&job).await {
                    Ok(runtime) => {
                        job = jobs::sync_job_runtime_status(&store, &job, &runtime).unwrap_or(job);
                        Some(runtime)
                    }
                    Err(_) => None,
                };
                if args.json {
                    println!(
                        "{}",
                        serde_json::to_string(&inspect::job_payload(job, runtime))?
                    );
                } else {
                    print!("{}", inspect::format_job_inspect(&job, runtime.as_ref()));
                }
            }
        }
        Commands::Status(args) => {
            let job = store.find_job(&args.job)?;
            let (synced_job, runtime) =
                crate::job_cli::resolve_and_sync_job_runtime(&store, job).await;
            println!(
                "{} {}",
                job_summary(&synced_job),
                crate::job_cli::runtime_suffix(&runtime)
            );
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
        Commands::Config(args) => {
            crate::config_cli::handle_config_command(&paths, config, args)?;
        }
        Commands::Completion(args) => {
            crate::completion::handle_completion_command(&store, args)?;
        }
    }
    Ok(())
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
    terminal_width_from_tty().or_else(terminal_width_from_env)
}

#[cfg(unix)]
fn terminal_width_from_tty() -> Option<usize> {
    let mut size = libc::winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let result = unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut size) };
    if result == 0 && size.ws_col >= 60 {
        Some(size.ws_col as usize)
    } else {
        None
    }
}

#[cfg(not(unix))]
fn terminal_width_from_tty() -> Option<usize> {
    None
}

fn terminal_width_from_env() -> Option<usize> {
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

fn ensure_import_confirmed(yes: bool, preview: &ImportPreview) -> Result<()> {
    if preview.would_import == 0 || yes {
        return Ok(());
    }
    if !io::stdin().is_terminal() {
        return Err(crate::StillrunError::invalid(
            "import-history requires confirmation; rerun with --yes after reviewing the preview",
        ));
    }
    if prompt_yes_no("Import these shell history entries into Stillrun? [y/N] ")? {
        Ok(())
    } else {
        Err(crate::StillrunError::invalid("import-history cancelled"))
    }
}

fn requires_replay_confirmation(record: &crate::db::ExecutionRecord) -> bool {
    record.status == ExecutionStatus::Imported || record.source.starts_with("shell:")
}

fn ensure_replay_confirmed(yes: bool, record: &crate::db::ExecutionRecord) -> Result<()> {
    if yes {
        return Ok(());
    }
    if !io::stdin().is_terminal() {
        return Err(crate::StillrunError::invalid(
            "replay of imported history requires confirmation; rerun with --preview or --yes",
        ));
    }
    eprint!("{}", inspect::format_replay_preview(record));
    if prompt_yes_no("Replay this imported history command? [y/N] ")? {
        Ok(())
    } else {
        Err(crate::StillrunError::invalid("replay cancelled"))
    }
}

fn prompt_yes_no(prompt: &str) -> Result<bool> {
    eprint!("{prompt}");
    io::stderr().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "YES" | "Yes"))
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
