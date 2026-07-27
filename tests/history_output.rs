use std::path::PathBuf;

use stillrun::{
    db::{ExecutionRecord, ExecutionStatus},
    output::{format_history_table, truncate_display, HistoryDisplayOptions},
};
use unicode_width::UnicodeWidthStr;

#[test]
fn truncates_history_rows_to_requested_width() {
    let command = "mflux-generate-qwen-edit --model ./qwen-edit-q6 --prompt \"将背景替换为咖啡厅室内，保持人物面部特征，光线自然\" --steps 8";
    let output = format_history_table(
        &[execution_record(command)],
        &HistoryDisplayOptions {
            width: 88,
            full: false,
            details: false,
        },
    );

    assert!(output.contains("ID"));
    assert!(output.contains("COMMAND"));
    assert!(output.contains("..."));
    assert!(output
        .lines()
        .all(|line| UnicodeWidthStr::width(line) <= 88));
}

#[test]
fn narrow_history_rows_stay_inside_minimum_table_width() {
    let output = format_history_table(
        &[execution_record("echo 将背景替换为咖啡厅室内光线自然")],
        &HistoryDisplayOptions {
            width: 60,
            full: false,
            details: false,
        },
    );

    assert!(output
        .lines()
        .all(|line| UnicodeWidthStr::width(line) <= 60));
}

#[test]
fn full_details_include_untruncated_command_and_metadata() {
    let command = "mflux-generate-qwen-edit --prompt \"将背景替换为咖啡厅室内，保持人物面部特征\"";
    let output = format_history_table(
        &[execution_record(command)],
        &HistoryDisplayOptions {
            width: 72,
            full: true,
            details: true,
        },
    );

    assert!(output.contains(command));
    assert!(output.contains("cwd: /tmp/stillrun-output"));
    assert!(output.contains("source: stillrun"));
}

#[test]
fn truncate_display_respects_wide_characters() {
    let truncated = truncate_display("abc将背景替换", 10);

    assert_eq!(UnicodeWidthStr::width(truncated.as_str()), 10);
    assert!(truncated.ends_with("..."));
}

fn execution_record(command: &str) -> ExecutionRecord {
    ExecutionRecord {
        id: 42,
        command: command.into(),
        argv: vec!["echo".into()],
        cwd: PathBuf::from("/tmp/stillrun-output"),
        git_repo: None,
        git_branch: None,
        git_head: None,
        started_at_ms: 1_700_000_000_000,
        ended_at_ms: Some(1_700_000_000_010),
        duration_ms: Some(10),
        exit_code: Some(0),
        status: ExecutionStatus::Success,
        env_json: "{}".into(),
        stdout: String::new(),
        stderr: String::new(),
        pid: Some(1234),
        background_job_id: None,
        restart_count: 0,
        source: "stillrun".into(),
        source_id: None,
    }
}
