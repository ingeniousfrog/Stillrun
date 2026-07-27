use std::{collections::BTreeMap, path::PathBuf};

use stillrun::{
    context::CommandContext,
    db::{ExecutionStatus, NewExecution, Store},
    jobs::background_request_from_execution,
    redact::REDACTED,
};

#[test]
fn builds_background_request_from_history_execution() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();
    let mut env = BTreeMap::new();
    env.insert("SAFE_FLAG".to_string(), "enabled".to_string());
    env.insert("API_TOKEN".to_string(), REDACTED.to_string());

    let id = store
        .insert_execution(&NewExecution {
            argv: vec!["/bin/sh".into(), "-c".into(), "echo promote".into()],
            context: CommandContext {
                cwd: PathBuf::from("/tmp/promote-project"),
                git_repo: Some(PathBuf::from("/tmp/promote-project")),
                git_branch: Some("main".into()),
                git_head: Some("abc123".into()),
                env,
            },
            started_at_ms: 10,
            ended_at_ms: Some(12),
            duration_ms: Some(2),
            exit_code: Some(0),
            status: ExecutionStatus::Success,
            stdout: "promote\n".into(),
            stderr: String::new(),
            pid: Some(7),
            background_job_id: None,
            restart_count: 0,
        })
        .unwrap();

    let request =
        background_request_from_execution(&store, id, Some("promoted".into()), true).unwrap();

    assert_eq!(request.name, Some("promoted".into()));
    assert_eq!(request.argv, vec!["/bin/sh", "-c", "echo promote"]);
    assert!(request.keep_alive);
    let context = request.context.unwrap();
    assert_eq!(context.cwd, PathBuf::from("/tmp/promote-project"));
    assert_eq!(context.env.get("SAFE_FLAG"), Some(&"enabled".to_string()));
    assert!(!context.env.contains_key("API_TOKEN"));
}
