use std::path::PathBuf;

use stillrun::{
    db::{ExecutionStatus, HistoryFilter, Store},
    history_import::ShellKind,
    shell_hook::{install_shell_hook_to_path, record_shell_hook_execution, ShellHookRecord},
};

#[test]
fn records_shell_hook_execution_with_context_and_exit_status() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();
    let cwd = temp.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();

    let inserted = record_shell_hook_execution(
        &store,
        ShellHookRecord {
            shell: ShellKind::Zsh,
            command: "mflux --prompt 咖啡厅室内光线".into(),
            cwd: cwd.clone(),
            started_at_ms: Some(1_000),
            exit_code: 7,
            source_id: Some("zsh-hook-1".into()),
        },
    )
    .unwrap();

    assert!(inserted.is_some());
    let matches = store
        .search_history(&HistoryFilter {
            query: Some("咖啡厅".into()),
            limit: 10,
            ..HistoryFilter::default()
        })
        .unwrap();

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].cwd, cwd);
    assert_eq!(matches[0].exit_code, Some(7));
    assert_eq!(matches[0].status, ExecutionStatus::Failed);
    assert_eq!(matches[0].source, "shell-hook:zsh");
}

#[test]
fn shell_hook_install_is_idempotent() {
    let temp = tempfile::tempdir().unwrap();
    let rc_path = temp.path().join(".zshrc");

    install_shell_hook_to_path(&rc_path, ShellKind::Zsh).unwrap();
    install_shell_hook_to_path(&rc_path, ShellKind::Zsh).unwrap();

    let text = std::fs::read_to_string(rc_path).unwrap();
    assert_eq!(text.matches("# >>> stillrun shell hook >>>").count(), 1);
    assert_eq!(text.matches("# <<< stillrun shell hook <<<").count(), 1);
    assert!(text.contains("stillrun hook record --shell zsh"));
}

#[test]
fn shell_hook_record_is_idempotent_for_same_source_id() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();

    for _ in 0..2 {
        record_shell_hook_execution(
            &store,
            ShellHookRecord {
                shell: ShellKind::Bash,
                command: "npm run dev".into(),
                cwd: PathBuf::from("/tmp/hook-idempotent"),
                started_at_ms: Some(2_000),
                exit_code: 0,
                source_id: Some("bash-hook-1".into()),
            },
        )
        .unwrap();
    }

    let matches = store
        .search_history(&HistoryFilter {
            query: Some("npm run dev".into()),
            limit: 10,
            ..HistoryFilter::default()
        })
        .unwrap();

    assert_eq!(matches.len(), 1);
}
