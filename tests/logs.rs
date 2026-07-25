use stillrun::logs::{prepare_follow_log_file, tail_log_file};

#[test]
fn tailing_missing_log_without_follow_returns_not_found() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("logs/missing.log");

    let error = tail_log_file(&missing, 10).unwrap_err();

    assert!(error.to_string().contains("not found"));
}

#[test]
fn preparing_follow_log_creates_missing_file_and_parent() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("logs/future.log");

    prepare_follow_log_file(&missing).unwrap();

    assert!(missing.exists());
    assert_eq!(tail_log_file(&missing, 10).unwrap(), "");
}

#[test]
fn tails_last_n_lines_with_trailing_newline() {
    let temp = tempfile::tempdir().unwrap();
    let log = temp.path().join("run.log");
    std::fs::write(&log, "one\ntwo\nthree\n").unwrap();

    let output = tail_log_file(&log, 2).unwrap();

    assert_eq!(output, "two\nthree\n");
}
