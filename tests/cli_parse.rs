use clap::Parser;
use stillrun::cli::{Cli, Commands};

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
