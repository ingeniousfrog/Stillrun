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
