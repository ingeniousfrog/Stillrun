use clap::Parser;
use stillrun::{
    cli::{Cli, Commands, HistorySort},
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
fn parses_history_sort_order() {
    let cli = Cli::try_parse_from(["stillrun", "history", "--sort", "oldest"]).unwrap();

    match cli.command {
        Commands::History(args) => assert_eq!(args.sort, HistorySort::Oldest),
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

#[test]
fn parses_replay_safety_flags() {
    let cli = Cli::try_parse_from(["stillrun", "replay", "42", "--preview", "--yes"]).unwrap();

    match cli.command {
        Commands::Replay(args) => {
            assert_eq!(args.id, 42);
            assert!(args.preview);
            assert!(args.yes);
        }
        other => panic!("expected replay command, got {other:?}"),
    }
}

#[test]
fn parses_import_history_preview_and_confirmation_flags() {
    let cli = Cli::try_parse_from([
        "stillrun",
        "import-history",
        "--shell",
        "zsh",
        "--file",
        "/tmp/zsh_history",
        "--preview",
        "--yes",
    ])
    .unwrap();

    match cli.command {
        Commands::ImportHistory(args) => {
            assert_eq!(args.shell, ImportShellSelection::Zsh);
            assert!(args.preview);
            assert!(args.yes);
        }
        other => panic!("expected import-history command, got {other:?}"),
    }
}

#[test]
fn parses_inspect_json_flag() {
    let cli = Cli::try_parse_from(["stillrun", "inspect", "dev-server", "--json"]).unwrap();

    match cli.command {
        Commands::Inspect(args) => {
            assert_eq!(args.target, "dev-server");
            assert!(args.json);
        }
        other => panic!("expected inspect command, got {other:?}"),
    }
}

#[test]
fn parses_config_commands() {
    let set =
        Cli::try_parse_from(["stillrun", "config", "set", "max-output-bytes", "4096"]).unwrap();
    let add = Cli::try_parse_from(["stillrun", "config", "redact", "add", "session"]).unwrap();

    match set.command {
        Commands::Config(args) => match args.action {
            stillrun::cli::ConfigCommand::Set(set_args) => {
                assert_eq!(set_args.key, "max-output-bytes");
                assert_eq!(set_args.value, "4096");
            }
            other => panic!("expected config set command, got {other:?}"),
        },
        other => panic!("expected config command, got {other:?}"),
    }

    match add.command {
        Commands::Config(args) => match args.action {
            stillrun::cli::ConfigCommand::Redact(redact_args) => match redact_args.action {
                stillrun::cli::ConfigRedactCommand::Add(add_args) => {
                    assert_eq!(add_args.key, "session");
                }
                other => panic!("expected config redact add command, got {other:?}"),
            },
            other => panic!("expected config redact command, got {other:?}"),
        },
        other => panic!("expected config command, got {other:?}"),
    }
}

#[test]
fn parses_completion_commands() {
    let script = Cli::try_parse_from(["stillrun", "completion", "zsh"]).unwrap();
    let candidates = Cli::try_parse_from([
        "stillrun",
        "completion",
        "candidates",
        "jobs",
        "--prefix",
        "dev",
    ])
    .unwrap();

    match script.command {
        Commands::Completion(args) => match args.action {
            stillrun::cli::CompletionCommand::Zsh => {}
            other => panic!("expected zsh completion command, got {other:?}"),
        },
        other => panic!("expected completion command, got {other:?}"),
    }

    match candidates.command {
        Commands::Completion(args) => match args.action {
            stillrun::cli::CompletionCommand::Candidates(candidate_args) => {
                assert_eq!(
                    candidate_args.kind,
                    stillrun::cli::CompletionCandidateKind::Jobs
                );
                assert_eq!(candidate_args.prefix, "dev");
            }
            other => panic!("expected completion candidates command, got {other:?}"),
        },
        other => panic!("expected completion command, got {other:?}"),
    }
}
