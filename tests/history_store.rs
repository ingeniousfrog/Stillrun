use std::{collections::BTreeMap, path::PathBuf};

use stillrun::{
    context::CommandContext,
    db::{ExecutionStatus, HistoryFilter, NewExecution, Store},
};

#[test]
fn stores_and_searches_execution_history_with_fts() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();

    let record = NewExecution {
        argv: vec!["echo".into(), "hello stillrun".into()],
        context: CommandContext {
            cwd: PathBuf::from("/tmp/project"),
            git_repo: Some(PathBuf::from("/tmp/project")),
            git_branch: Some("main".into()),
            env: BTreeMap::new(),
        },
        started_at_ms: 10,
        ended_at_ms: Some(15),
        duration_ms: Some(5),
        exit_code: Some(0),
        status: ExecutionStatus::Success,
        stdout: "hello stillrun\n".into(),
        stderr: String::new(),
        pid: Some(42),
        background_job_id: None,
        restart_count: 0,
    };

    let id = store.insert_execution(&record).unwrap();
    let matches = store
        .search_history(&HistoryFilter {
            query: Some("stillrun".into()),
            cwd: Some(PathBuf::from("/tmp/project")),
            repo: Some(PathBuf::from("/tmp/project")),
            status: Some(ExecutionStatus::Success),
            limit: 10,
            ..HistoryFilter::default()
        })
        .unwrap();

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].id, id);
    assert_eq!(matches[0].argv, vec!["echo", "hello stillrun"]);
}

#[test]
fn returns_execution_by_id_for_replay() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();

    let id = store
        .insert_execution(&NewExecution {
            argv: vec!["printf".into(), "hello".into()],
            context: CommandContext {
                cwd: temp.path().to_path_buf(),
                git_repo: None,
                git_branch: None,
                env: BTreeMap::new(),
            },
            started_at_ms: 100,
            ended_at_ms: Some(101),
            duration_ms: Some(1),
            exit_code: Some(0),
            status: ExecutionStatus::Success,
            stdout: "hello".into(),
            stderr: String::new(),
            pid: None,
            background_job_id: None,
            restart_count: 0,
        })
        .unwrap();

    let fetched = store.get_execution(id).unwrap();
    assert_eq!(fetched.argv, vec!["printf", "hello"]);
}

#[test]
fn redacts_sensitive_argv_before_persisting_history() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();

    let id = store
        .insert_execution(&NewExecution {
            argv: vec![
                "curl".into(),
                "--token".into(),
                "super-secret".into(),
                "--password=hunter2".into(),
            ],
            context: CommandContext {
                cwd: temp.path().to_path_buf(),
                git_repo: None,
                git_branch: None,
                env: BTreeMap::new(),
            },
            started_at_ms: 100,
            ended_at_ms: Some(101),
            duration_ms: Some(1),
            exit_code: Some(0),
            status: ExecutionStatus::Success,
            stdout: String::new(),
            stderr: String::new(),
            pid: None,
            background_job_id: None,
            restart_count: 0,
        })
        .unwrap();

    let fetched = store.get_execution(id).unwrap();
    assert_eq!(fetched.argv[2], stillrun::redact::REDACTED);
    assert_eq!(
        fetched.argv[3],
        format!("--password={}", stillrun::redact::REDACTED)
    );
    assert!(!fetched.command.contains("super-secret"));
    assert!(!fetched.command.contains("hunter2"));
}

#[test]
fn filters_execution_history_by_started_time_range() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();

    insert_test_execution(&store, "old", 1_000);
    let expected = insert_test_execution(&store, "middle", 2_000);
    insert_test_execution(&store, "new", 3_000);

    let matches = store
        .search_history(&HistoryFilter {
            started_after_ms: Some(1_500),
            started_before_ms: Some(2_500),
            limit: 10,
            ..HistoryFilter::default()
        })
        .unwrap();

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].id, expected);
    assert_eq!(matches[0].argv, vec!["echo", "middle"]);
}

#[test]
fn blank_query_behaves_like_unfiltered_history() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();
    let expected = insert_test_execution(&store, "blank-query", 4_000);

    let matches = store
        .search_history(&HistoryFilter {
            query: Some("\n \t".into()),
            limit: 10,
            ..HistoryFilter::default()
        })
        .unwrap();

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].id, expected);
}

#[test]
fn searches_chinese_substrings_without_fts_tokenization_help() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();
    let expected = store
        .insert_execution(&NewExecution {
            argv: vec![
                "mflux-generate-qwen-edit".into(),
                "--prompt".into(),
                "将背景替换为咖啡厅室内，保持人物面部特征".into(),
            ],
            context: CommandContext {
                cwd: PathBuf::from("/tmp/chinese-search"),
                git_repo: None,
                git_branch: None,
                env: BTreeMap::new(),
            },
            started_at_ms: 5_000,
            ended_at_ms: Some(5_001),
            duration_ms: Some(1),
            exit_code: Some(0),
            status: ExecutionStatus::Success,
            stdout: String::new(),
            stderr: String::new(),
            pid: None,
            background_job_id: None,
            restart_count: 0,
        })
        .unwrap();

    let matches = store
        .search_history(&HistoryFilter {
            query: Some("咖啡厅".into()),
            limit: 10,
            ..HistoryFilter::default()
        })
        .unwrap();

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].id, expected);
}

#[test]
fn deletes_one_execution_and_updates_history_search() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();
    let id = insert_test_execution(&store, "delete-me", 6_000);

    assert!(store.delete_execution(id).unwrap());
    assert!(!store.delete_execution(id).unwrap());
    let matches = store
        .search_history(&HistoryFilter {
            query: Some("delete-me".into()),
            limit: 10,
            ..HistoryFilter::default()
        })
        .unwrap();

    assert!(matches.is_empty());
}

#[test]
fn clears_only_imported_history_records() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();
    let kept = insert_test_execution(&store, "native-record", 7_000);
    insert_imported_test_execution(&store, "imported-record", 7_001, "1");

    let deleted = store.clear_imported_history().unwrap();

    assert_eq!(deleted, 1);
    assert!(store.get_execution(kept).is_ok());
    assert!(store
        .search_history(&HistoryFilter {
            query: Some("imported-record".into()),
            limit: 10,
            ..HistoryFilter::default()
        })
        .unwrap()
        .is_empty());
}

#[test]
fn prunes_history_before_timestamp_with_optional_source() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();
    let kept_native = insert_test_execution(&store, "native-old", 8_000);
    insert_imported_test_execution(&store, "imported-old", 8_000, "2");
    let kept_imported = insert_imported_test_execution(&store, "imported-new", 9_000, "3");

    let deleted = store
        .prune_history_before(8_500, Some("shell:zsh:/tmp/history"))
        .unwrap();

    assert_eq!(deleted, 1);
    assert!(store.get_execution(kept_native).is_ok());
    assert!(store.get_execution(kept_imported).is_ok());
}

fn insert_test_execution(store: &Store, label: &str, started_at_ms: i64) -> i64 {
    store
        .insert_execution(&NewExecution {
            argv: vec!["echo".into(), label.into()],
            context: CommandContext {
                cwd: PathBuf::from("/tmp/history-time"),
                git_repo: None,
                git_branch: None,
                env: BTreeMap::new(),
            },
            started_at_ms,
            ended_at_ms: Some(started_at_ms + 1),
            duration_ms: Some(1),
            exit_code: Some(0),
            status: ExecutionStatus::Success,
            stdout: format!("{label}\n"),
            stderr: String::new(),
            pid: None,
            background_job_id: None,
            restart_count: 0,
        })
        .unwrap()
}

fn insert_imported_test_execution(
    store: &Store,
    label: &str,
    started_at_ms: i64,
    source_id: &str,
) -> i64 {
    store
        .insert_imported_execution(
            &NewExecution {
                argv: vec!["zsh".into(), "-lc".into(), label.into()],
                context: CommandContext {
                    cwd: PathBuf::from("/tmp/history"),
                    git_repo: None,
                    git_branch: None,
                    env: BTreeMap::new(),
                },
                started_at_ms,
                ended_at_ms: None,
                duration_ms: None,
                exit_code: None,
                status: ExecutionStatus::Imported,
                stdout: String::new(),
                stderr: String::new(),
                pid: None,
                background_job_id: None,
                restart_count: 0,
            },
            "shell:zsh:/tmp/history",
            source_id,
            label,
        )
        .unwrap()
        .unwrap()
}
