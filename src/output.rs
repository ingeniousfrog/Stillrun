use crate::db::{ExecutionRecord, JobRecord};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryDisplayOptions {
    pub width: usize,
    pub full: bool,
    pub details: bool,
}

pub fn execution_summary(record: &ExecutionRecord) -> String {
    format!(
        "#{:<4} {:<10} {}",
        record.id,
        record.status.as_str(),
        record.command
    )
}

pub fn format_history_table(
    records: &[ExecutionRecord],
    options: &HistoryDisplayOptions,
) -> String {
    if records.is_empty() {
        return String::new();
    }

    let width = options.width.max(60);
    let fixed_width = 6 + 1 + 10 + 1 + 6 + 1 + 20 + 1;
    let command_width = width.saturating_sub(fixed_width).max(1);
    let mut output = String::new();
    output.push_str(&format!(
        "{} {} {} {} {}\n",
        pad_display("ID", 6),
        pad_display("STATUS", 10),
        pad_display("EXIT", 6),
        pad_display("CWD", 20),
        "COMMAND"
    ));
    output.push_str(&format!("{}\n", "-".repeat(width)));

    for record in records {
        let id = format!("#{}", record.id);
        let exit = record
            .exit_code
            .map(|code| code.to_string())
            .unwrap_or_else(|| "-".into());
        let cwd = record.cwd.to_string_lossy();
        let command = if options.full {
            record.command.clone()
        } else {
            truncate_display(&record.command, command_width)
        };
        output.push_str(&format!(
            "{} {} {} {} {}\n",
            pad_display(&id, 6),
            pad_display(record.status.as_str(), 10),
            pad_display(&exit, 6),
            pad_display(&truncate_display(&cwd, 20), 20),
            command
        ));

        if options.details {
            append_history_details(&mut output, record);
        }
    }

    output
}

pub fn truncate_display(value: &str, max_width: usize) -> String {
    let width = UnicodeWidthStr::width(value);
    if width <= max_width {
        return value.to_string();
    }
    if max_width <= 3 {
        return ".".repeat(max_width);
    }

    let target = max_width - 3;
    let mut result = String::new();
    let mut used = 0;
    for ch in value.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + ch_width > target {
            break;
        }
        result.push(ch);
        used += ch_width;
    }
    result.push_str("...");
    result
}

pub fn job_summary(record: &JobRecord) -> String {
    format!(
        "{} {:<10} {}",
        record.id,
        record.status.as_str(),
        record.command
    )
}

fn append_history_details(output: &mut String, record: &ExecutionRecord) {
    output.push_str(&format!("  command: {}\n", record.command));
    output.push_str(&format!("  cwd: {}\n", record.cwd.display()));
    if let Some(repo) = &record.git_repo {
        output.push_str(&format!("  git repo: {}\n", repo.display()));
    }
    if let Some(branch) = &record.git_branch {
        output.push_str(&format!("  git branch: {branch}\n"));
    }
    if let Some(head) = &record.git_head {
        output.push_str(&format!("  git head: {head}\n"));
    }
    output.push_str(&format!("  source: {}", record.source));
    if let Some(source_id) = &record.source_id {
        output.push_str(&format!(":{source_id}"));
    }
    output.push('\n');
    if let Some(duration_ms) = record.duration_ms {
        output.push_str(&format!("  duration ms: {duration_ms}\n"));
    }
    if let Some(pid) = record.pid {
        output.push_str(&format!("  pid: {pid}\n"));
    }
}

fn pad_display(value: &str, width: usize) -> String {
    let current = UnicodeWidthStr::width(value);
    if current >= width {
        return truncate_display(value, width);
    }
    format!("{value}{}", " ".repeat(width - current))
}
