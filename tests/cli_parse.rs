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
