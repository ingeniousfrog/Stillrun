use stillrun::{db::JobStatus, jobs::status};

#[test]
fn parses_launchd_running_pid() {
    let parsed = status::parse_launchd_print(
        r#"
        state = running
        runs = 4
        pid = 4821
        "#,
    );

    assert_eq!(parsed.status, JobStatus::Running);
    assert_eq!(parsed.pid, Some(4821));
    assert_eq!(parsed.restart_count, Some(3));
}

#[test]
fn parses_launchd_stopped_job() {
    let parsed = status::parse_launchd_print(
        r#"
        state = waiting
        last exit code = 0
        "#,
    );

    assert_eq!(parsed.status, JobStatus::Stopped);
    assert_eq!(parsed.pid, None);
}

#[test]
fn parses_launchd_failed_exit_code() {
    let parsed = status::parse_launchd_print(
        r#"
        state = waiting
        last exit code = 7
        "#,
    );

    assert_eq!(parsed.status, JobStatus::Failed);
    assert_eq!(parsed.pid, None);
    assert_eq!(parsed.last_exit_code, Some(7));
}

#[test]
fn parses_process_resource_sample() {
    let parsed = status::parse_ps_output("  1.5  12345\n");

    assert_eq!(parsed, Some((1.5, 12345)));
}

#[test]
fn parses_process_tree_resources_for_parent_and_children() {
    let parsed = status::parse_ps_tree_output(
        r#"
          10      1   1.0   1000
          11     10   2.5   2000
          12     11   3.0   3000
          20      1  99.0  90000
        "#,
        10,
    );

    assert_eq!(parsed, Some((6.5, 6000)));
}

#[test]
fn parses_descendant_pids_without_including_unrelated_processes() {
    let parsed = status::parse_descendant_pids(
        r#"
          10      1
          11     10
          12     11
          20      1
        "#,
        10,
    );

    assert_eq!(parsed, vec![11, 12]);
}
