use clap::Parser;
use stillrun::{
    cli::{Cli, Commands},
    history_import::ImportShellSelection,
};

#[test]
fn parses_start_job_command() {
    let cli = Cli::try_parse_from(["stillrun", "start", "dev-server"]).unwrap();

    match cli.command {
        Commands::Start(args) => assert_eq!(args.job, "dev-server"),
        other => panic!("expected start command, got {other:?}"),
    }
}

#[test]
fn parses_status_job_command() {
    let cli = Cli::try_parse_from(["stillrun", "status", "dev-server"]).unwrap();

    match cli.command {
        Commands::Status(args) => assert_eq!(args.job, "dev-server"),
        other => panic!("expected status command, got {other:?}"),
    }
}

#[test]
fn parses_import_history_command() {
    let cli = Cli::try_parse_from([
        "stillrun",
        "import-history",
        "--shell",
        "zsh",
        "--file",
        "/tmp/zsh_history",
    ])
    .unwrap();

    match cli.command {
        Commands::ImportHistory(args) => {
            assert_eq!(args.shell, ImportShellSelection::Zsh);
            assert_eq!(args.file.unwrap().to_string_lossy(), "/tmp/zsh_history");
        }
        other => panic!("expected import-history command, got {other:?}"),
    }
}

#[test]
fn parses_history_display_flags() {
    let cli = Cli::try_parse_from([
        "stillrun",
        "history",
        "--query",
        "咖啡厅",
        "--full",
        "--details",
        "--pager",
        "--width",
        "90",
    ])
    .unwrap();

    match cli.command {
        Commands::History(args) => {
            assert_eq!(args.query.as_deref(), Some("咖啡厅"));
            assert!(args.full);
            assert!(args.details);
            assert!(args.pager);
            assert_eq!(args.width, Some(90));
        }
        other => panic!("expected history command, got {other:?}"),
    }
}

#[test]
fn parses_history_delete_command() {
    let cli = Cli::try_parse_from(["stillrun", "history", "delete", "42"]).unwrap();

    match cli.command {
        Commands::History(args) => match args.action.unwrap() {
            stillrun::cli::HistoryCommand::Delete(delete_args) => assert_eq!(delete_args.id, 42),
            other => panic!("expected history delete command, got {other:?}"),
        },
        other => panic!("expected history command, got {other:?}"),
    }
}

#[test]
fn parses_history_clear_command() {
    let cli = Cli::try_parse_from(["stillrun", "history", "clear", "--imported", "--yes"]).unwrap();

    match cli.command {
        Commands::History(args) => match args.action.unwrap() {
            stillrun::cli::HistoryCommand::Clear(clear_args) => {
                assert!(clear_args.imported);
                assert!(clear_args.yes);
            }
            other => panic!("expected history clear command, got {other:?}"),
        },
        other => panic!("expected history command, got {other:?}"),
    }
}

#[test]
fn parses_history_prune_command() {
    let cli = Cli::try_parse_from([
        "stillrun",
        "history",
        "prune",
        "--before-ms",
        "1700000000000",
        "--source",
        "shell-hook:zsh",
        "--yes",
    ])
    .unwrap();

    match cli.command {
        Commands::History(args) => match args.action.unwrap() {
            stillrun::cli::HistoryCommand::Prune(prune_args) => {
                assert_eq!(prune_args.before_ms, 1_700_000_000_000);
                assert_eq!(prune_args.source.as_deref(), Some("shell-hook:zsh"));
                assert!(prune_args.yes);
            }
            other => panic!("expected history prune command, got {other:?}"),
        },
        other => panic!("expected history command, got {other:?}"),
    }
}

#[test]
fn parses_jobs_delete_command() {
    let cli = Cli::try_parse_from(["stillrun", "jobs", "delete", "dev", "--keep-plist"]).unwrap();

    match cli.command {
        Commands::Jobs(args) => match args.action.unwrap() {
            stillrun::cli::JobsCommand::Delete(delete_args) => {
                assert_eq!(delete_args.job, "dev");
                assert!(delete_args.keep_plist);
            }
            other => panic!("expected jobs delete command, got {other:?}"),
        },
        other => panic!("expected jobs command, got {other:?}"),
    }
}

#[test]
fn parses_jobs_monitor_command() {
    let cli = Cli::try_parse_from([
        "stillrun",
        "jobs",
        "monitor",
        "dev",
        "--once",
        "--interval-secs",
        "1",
        "--cpu-alert",
        "80",
        "--rss-alert-mb",
        "512",
    ])
    .unwrap();

    match cli.command {
        Commands::Jobs(args) => match args.action.unwrap() {
            stillrun::cli::JobsCommand::Monitor(monitor_args) => {
                assert_eq!(monitor_args.job, "dev");
                assert!(monitor_args.once);
                assert_eq!(monitor_args.interval_secs, 1);
                assert_eq!(monitor_args.cpu_alert, Some(80.0));
                assert_eq!(monitor_args.rss_alert_mb, Some(512));
            }
            other => panic!("expected jobs monitor command, got {other:?}"),
        },
        other => panic!("expected jobs command, got {other:?}"),
    }
}

#[test]
fn parses_jobs_events_and_samples_commands() {
    let events = Cli::try_parse_from([
        "stillrun", "jobs", "events", "dev", "--follow", "--limit", "20",
    ])
    .unwrap();
    let samples =
        Cli::try_parse_from(["stillrun", "jobs", "samples", "dev", "--limit", "10"]).unwrap();

    match events.command {
        Commands::Jobs(args) => match args.action.unwrap() {
            stillrun::cli::JobsCommand::Events(events_args) => {
                assert_eq!(events_args.job, "dev");
                assert!(events_args.follow);
                assert_eq!(events_args.limit, 20);
            }
            other => panic!("expected jobs events command, got {other:?}"),
        },
        other => panic!("expected jobs command, got {other:?}"),
    }

    match samples.command {
        Commands::Jobs(args) => match args.action.unwrap() {
            stillrun::cli::JobsCommand::Samples(samples_args) => {
                assert_eq!(samples_args.job, "dev");
                assert_eq!(samples_args.limit, 10);
            }
            other => panic!("expected jobs samples command, got {other:?}"),
        },
        other => panic!("expected jobs command, got {other:?}"),
    }
}

#[test]
fn parses_hook_record_command() {
    let cli = Cli::try_parse_from([
        "stillrun",
        "hook",
        "record",
        "--shell",
        "zsh",
        "--exit-code",
        "2",
        "--cwd",
        "/tmp/project",
        "--started-ms",
        "1000",
        "--",
        "echo 咖啡厅",
    ])
    .unwrap();

    match cli.command {
        Commands::Hook(args) => match args.action {
            stillrun::cli::HookCommand::Record(record_args) => {
                assert_eq!(record_args.exit_code, 2);
                assert_eq!(record_args.cwd.to_string_lossy(), "/tmp/project");
                assert_eq!(record_args.command, vec!["echo 咖啡厅"]);
            }
            other => panic!("expected hook record command, got {other:?}"),
        },
        other => panic!("expected hook command, got {other:?}"),
    }
}
