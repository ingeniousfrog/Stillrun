use crate::db::{ExecutionRecord, JobRecord};

pub fn execution_summary(record: &ExecutionRecord) -> String {
    format!(
        "#{:<4} {:<10} {}",
        record.id,
        record.status.as_str(),
        record.command
    )
}

pub fn job_summary(record: &JobRecord) -> String {
    format!(
        "{} {:<10} {}",
        record.id,
        record.status.as_str(),
        record.command
    )
}
