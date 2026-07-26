use std::time::Duration;

use clap::{Args, Subcommand};

use crate::{
    db::{JobRecord, Store},
    job_view::{
        build_job_dashboard, format_job_dashboard, format_job_list, format_job_timeline,
        format_job_timeline_event, JobListEntry,
    },
    jobs::{self, status::RuntimeJobStatus},
    output::job_summary,
    Result,
};

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

pub async fn handle_jobs_command(store: &Store, args: JobsArgs) -> Result<()> {
    match args.action {
        Some(JobsCommand::Delete(delete_args)) => {
            let report =
                jobs::delete_job_with_report(store, &delete_args.job, delete_args.keep_plist)
                    .await?;
            if let Some(warning) = &report.launchd_warning {
                eprintln!("warning: {warning}");
            }
            let job = report.job;
            println!("deleted job {} plist={}", job.id, job.plist_path.display());
        }
        Some(JobsCommand::Monitor(monitor_args)) => {
            monitor_job(store, monitor_args).await?;
        }
        Some(JobsCommand::Events(events_args)) => {
            print_job_events(store, events_args).await?;
        }
        Some(JobsCommand::Samples(samples_args)) => {
            let job = store.find_job(&samples_args.job)?;
            for sample in store.list_job_resource_samples(&job.id, samples_args.limit)? {
                println!("{}", format_job_sample(&sample));
            }
        }
        None => {
            let mut entries = Vec::new();
            for job in store.list_jobs()? {
                let (synced_job, runtime) = resolve_and_sync_job_runtime(store, job).await;
                entries.push(job_list_entry(store, synced_job, runtime)?);
            }
            print!("{}", format_job_list(&entries));
        }
    }
    Ok(())
}

pub async fn resolve_and_sync_job_runtime(
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

pub fn runtime_suffix(runtime: &RuntimeJobStatus) -> String {
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
        if args.once {
            let dashboard = build_job_dashboard(store, synced_job, runtime)?;
            print!("{}", format_job_dashboard(&dashboard));
            println!("Alerts: {alert_count}");
            return Ok(());
        }
        println!(
            "{} {} alerts={alert_count}",
            job_summary(&synced_job),
            runtime_suffix(&runtime)
        );
        tokio::time::sleep(interval).await;
    }
}

async fn print_job_events(store: &Store, args: JobEventsArgs) -> Result<()> {
    let job = store.find_job(&args.job)?;
    let mut after_id = None;
    let events = store.list_job_events(&job.id, args.limit)?;
    for event in &events {
        after_id = Some(after_id.map_or(event.id, |current: i64| current.max(event.id)));
    }
    print!("{}", format_job_timeline(&job, &events));
    if !args.follow {
        return Ok(());
    }

    let interval = Duration::from_secs(args.interval_secs.max(1));
    loop {
        tokio::time::sleep(interval).await;
        for event in store.list_job_events_after(&job.id, after_id, args.limit)? {
            after_id = Some(after_id.map_or(event.id, |current: i64| current.max(event.id)));
            println!("  {}", format_job_timeline_event(&event));
        }
    }
}

fn job_list_entry(
    store: &Store,
    job: JobRecord,
    runtime: RuntimeJobStatus,
) -> Result<JobListEntry> {
    Ok(JobListEntry {
        last_sample: store
            .list_job_resource_samples(&job.id, 1)?
            .into_iter()
            .next(),
        job,
        runtime,
    })
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
