use stillrun::logs::{prepare_follow_log_file, rotate_log_file, tail_log_file};

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

#[test]
fn rotates_log_file_when_threshold_is_exceeded() {
    let temp = tempfile::tempdir().unwrap();
    let log = temp.path().join("run.log");
    std::fs::write(&log, "one\ntwo\nthree\n").unwrap();

    let report = rotate_log_file(&log, 8).unwrap();

    assert!(report.rotated);
    assert_eq!(report.rotated_path.unwrap(), temp.path().join("run.log.1"));
    assert_eq!(
        std::fs::read_to_string(temp.path().join("run.log.1")).unwrap(),
        "one\ntwo\nthree\n"
    );
    assert_eq!(std::fs::read_to_string(&log).unwrap(), "");
}

#[test]
fn skips_log_rotation_when_file_is_under_threshold() {
    let temp = tempfile::tempdir().unwrap();
    let log = temp.path().join("run.log");
    std::fs::write(&log, "short\n").unwrap();

    let report = rotate_log_file(&log, 100).unwrap();

    assert!(!report.rotated);
    assert!(report.rotated_path.is_none());
    assert_eq!(std::fs::read_to_string(&log).unwrap(), "short\n");
}
