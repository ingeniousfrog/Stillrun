use std::path::{Path, PathBuf};

use rusqlite::{
    params, params_from_iter,
    types::{ToSql, Value},
    Connection,
};
use serde::{Deserialize, Serialize};

use crate::{context::CommandContext, redact, Result, StillrunError};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionRecord {
    pub id: i64,
    pub command: String,
    pub argv: Vec<String>,
    pub cwd: PathBuf,
    pub git_repo: Option<PathBuf>,
    pub git_branch: Option<String>,
    pub started_at_ms: i64,
    pub ended_at_ms: Option<i64>,
    pub duration_ms: Option<i64>,
    pub exit_code: Option<i32>,
    pub status: ExecutionStatus,
    pub env_json: String,
    pub stdout: String,
    pub stderr: String,
    pub pid: Option<u32>,
    pub background_job_id: Option<String>,
    pub restart_count: i64,
    pub source: String,
    pub source_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExecutionStatus {
    Running,
    Success,
    Failed,
    Background,
    Imported,
}

impl ExecutionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Background => "background",
            Self::Imported => "imported",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "running" => Ok(Self::Running),
            "success" => Ok(Self::Success),
            "failed" => Ok(Self::Failed),
            "background" => Ok(Self::Background),
            "imported" => Ok(Self::Imported),
            other => Err(StillrunError::invalid(format!(
                "unknown execution status '{other}'"
            ))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NewExecution {
    pub argv: Vec<String>,
    pub context: CommandContext,
    pub started_at_ms: i64,
    pub ended_at_ms: Option<i64>,
    pub duration_ms: Option<i64>,
    pub exit_code: Option<i32>,
    pub status: ExecutionStatus,
    pub stdout: String,
    pub stderr: String,
    pub pid: Option<u32>,
    pub background_job_id: Option<String>,
    pub restart_count: i64,
}

#[derive(Debug, Clone, Default)]
pub struct HistoryFilter {
    pub query: Option<String>,
    pub cwd: Option<PathBuf>,
    pub repo: Option<PathBuf>,
    pub status: Option<ExecutionStatus>,
    pub started_after_ms: Option<i64>,
    pub started_before_ms: Option<i64>,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JobRecord {
    pub id: String,
    pub name: String,
    pub label: String,
    pub argv: Vec<String>,
    pub command: String,
    pub cwd: PathBuf,
    pub git_repo: Option<PathBuf>,
    pub git_branch: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub status: JobStatus,
    pub pid: Option<u32>,
    pub restart_count: i64,
    pub stdout_path: PathBuf,
    pub stderr_path: PathBuf,
    pub plist_path: PathBuf,
    pub keep_alive: bool,
    pub last_exit_code: Option<i32>,
    pub last_cpu_percent: Option<f32>,
    pub last_rss_kb: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JobRuntimeUpdate {
    pub status: JobStatus,
    pub pid: Option<u32>,
    pub last_exit_code: Option<i32>,
    pub cpu_percent: Option<f32>,
    pub rss_kb: Option<u64>,
    pub restart_count: Option<i64>,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum JobStatus {
    Created,
    Running,
    Stopped,
    Failed,
    Unknown,
}

impl JobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Running => "running",
            Self::Stopped => "stopped",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "created" => Ok(Self::Created),
            "running" => Ok(Self::Running),
            "stopped" => Ok(Self::Stopped),
            "failed" => Ok(Self::Failed),
            "unknown" => Ok(Self::Unknown),
            other => Err(StillrunError::invalid(format!(
                "unknown job status '{other}'"
            ))),
        }
    }
}

pub struct Store {
    conn: Connection,
}

struct PreparedExecution {
    command: String,
    argv_json: String,
    env_json: String,
    stdout: String,
    stderr: String,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        Ok(Self { conn })
    }

    pub fn initialize(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS executions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                command TEXT NOT NULL,
                argv_json TEXT NOT NULL,
                cwd TEXT NOT NULL,
                git_repo TEXT,
                git_branch TEXT,
                started_at_ms INTEGER NOT NULL,
                ended_at_ms INTEGER,
                duration_ms INTEGER,
                exit_code INTEGER,
                status TEXT NOT NULL,
                env_json TEXT NOT NULL,
                stdout TEXT NOT NULL,
                stderr TEXT NOT NULL,
                pid INTEGER,
                background_job_id TEXT,
                restart_count INTEGER NOT NULL DEFAULT 0,
                source TEXT NOT NULL DEFAULT 'stillrun',
                source_id TEXT
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS executions_fts USING fts5(
                command,
                cwd,
                stdout,
                stderr,
                content='executions',
                content_rowid='id'
            );

            CREATE TRIGGER IF NOT EXISTS executions_ai AFTER INSERT ON executions BEGIN
                INSERT INTO executions_fts(rowid, command, cwd, stdout, stderr)
                VALUES (new.id, new.command, new.cwd, new.stdout, new.stderr);
            END;

            CREATE TRIGGER IF NOT EXISTS executions_ad AFTER DELETE ON executions BEGIN
                INSERT INTO executions_fts(executions_fts, rowid, command, cwd, stdout, stderr)
                VALUES ('delete', old.id, old.command, old.cwd, old.stdout, old.stderr);
            END;

            CREATE TRIGGER IF NOT EXISTS executions_au AFTER UPDATE ON executions BEGIN
                INSERT INTO executions_fts(executions_fts, rowid, command, cwd, stdout, stderr)
                VALUES ('delete', old.id, old.command, old.cwd, old.stdout, old.stderr);
                INSERT INTO executions_fts(rowid, command, cwd, stdout, stderr)
                VALUES (new.id, new.command, new.cwd, new.stdout, new.stderr);
            END;

            CREATE TABLE IF NOT EXISTS jobs (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                label TEXT NOT NULL UNIQUE,
                argv_json TEXT NOT NULL,
                command TEXT NOT NULL,
                cwd TEXT NOT NULL,
                git_repo TEXT,
                git_branch TEXT,
                created_at_ms INTEGER NOT NULL,
                updated_at_ms INTEGER NOT NULL,
                status TEXT NOT NULL,
                pid INTEGER,
                restart_count INTEGER NOT NULL DEFAULT 0,
                stdout_path TEXT NOT NULL,
                stderr_path TEXT NOT NULL,
                plist_path TEXT NOT NULL,
                keep_alive INTEGER NOT NULL DEFAULT 0,
                last_exit_code INTEGER,
                last_cpu_percent REAL,
                last_rss_kb INTEGER
            );

            CREATE INDEX IF NOT EXISTS idx_executions_started_at ON executions(started_at_ms DESC);
            CREATE INDEX IF NOT EXISTS idx_executions_cwd ON executions(cwd);
            CREATE INDEX IF NOT EXISTS idx_executions_repo ON executions(git_repo);
            CREATE INDEX IF NOT EXISTS idx_executions_status ON executions(status);
            CREATE INDEX IF NOT EXISTS idx_jobs_name ON jobs(name);
            "#,
        )?;
        self.ensure_executions_column("source", "TEXT NOT NULL DEFAULT 'stillrun'")?;
        self.ensure_executions_column("source_id", "TEXT")?;
        self.conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_executions_source_id ON executions(source, source_id) WHERE source_id IS NOT NULL",
            [],
        )?;
        self.ensure_jobs_column("keep_alive", "INTEGER NOT NULL DEFAULT 0")?;
        self.ensure_jobs_column("last_cpu_percent", "REAL")?;
        self.ensure_jobs_column("last_rss_kb", "INTEGER")?;
        Ok(())
    }

    pub fn insert_execution(&self, record: &NewExecution) -> Result<i64> {
        self.insert_execution_internal(record, "stillrun", None, None, false)
            .map(|id| id.expect("normal executions are never ignored"))
    }

    pub fn insert_imported_execution(
        &self,
        record: &NewExecution,
        source: &str,
        source_id: &str,
        command: &str,
    ) -> Result<Option<i64>> {
        let inserted =
            self.insert_execution_internal(record, source, Some(source_id), Some(command), true)?;
        if inserted.is_none() {
            self.refresh_imported_execution(record, source, source_id, command)?;
        }
        Ok(inserted)
    }

    fn insert_execution_internal(
        &self,
        record: &NewExecution,
        source: &str,
        source_id: Option<&str>,
        command_override: Option<&str>,
        ignore_duplicate: bool,
    ) -> Result<Option<i64>> {
        let prepared = prepare_execution(record, command_override)?;
        let insert_clause = if ignore_duplicate {
            "INSERT OR IGNORE"
        } else {
            "INSERT"
        };

        self.conn.execute(
            &format!(
                r#"
            {insert_clause} INTO executions (
                command, argv_json, cwd, git_repo, git_branch, started_at_ms, ended_at_ms,
                duration_ms, exit_code, status, env_json, stdout, stderr, pid,
                background_job_id, restart_count, source, source_id
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
            "#
            ),
            params![
                prepared.command,
                prepared.argv_json,
                record.context.cwd.to_string_lossy().to_string(),
                record
                    .context
                    .git_repo
                    .as_ref()
                    .map(|path| path.to_string_lossy().to_string()),
                record.context.git_branch,
                record.started_at_ms,
                record.ended_at_ms,
                record.duration_ms,
                record.exit_code,
                record.status.as_str(),
                prepared.env_json,
                prepared.stdout,
                prepared.stderr,
                record.pid.map(i64::from),
                record.background_job_id,
                record.restart_count,
                source,
                source_id,
            ],
        )?;
        if ignore_duplicate && self.conn.changes() == 0 {
            return Ok(None);
        }
        Ok(Some(self.conn.last_insert_rowid()))
    }

    fn refresh_imported_execution(
        &self,
        record: &NewExecution,
        source: &str,
        source_id: &str,
        command_override: &str,
    ) -> Result<()> {
        let prepared = prepare_execution(record, Some(command_override))?;
        self.conn.execute(
            r#"
            UPDATE executions
            SET command = ?3,
                argv_json = ?4,
                cwd = ?5,
                git_repo = ?6,
                git_branch = ?7,
                started_at_ms = ?8,
                ended_at_ms = ?9,
                duration_ms = ?10,
                exit_code = ?11,
                status = ?12,
                env_json = ?13,
                stdout = ?14,
                stderr = ?15,
                pid = ?16,
                background_job_id = ?17,
                restart_count = ?18
            WHERE source = ?1 AND source_id = ?2 AND status = 'imported'
            "#,
            params![
                source,
                source_id,
                prepared.command,
                prepared.argv_json,
                record.context.cwd.to_string_lossy().to_string(),
                record
                    .context
                    .git_repo
                    .as_ref()
                    .map(|path| path.to_string_lossy().to_string()),
                record.context.git_branch,
                record.started_at_ms,
                record.ended_at_ms,
                record.duration_ms,
                record.exit_code,
                record.status.as_str(),
                prepared.env_json,
                prepared.stdout,
                prepared.stderr,
                record.pid.map(i64::from),
                record.background_job_id,
                record.restart_count,
            ],
        )?;
        Ok(())
    }

    pub fn get_execution(&self, _id: i64) -> Result<ExecutionRecord> {
        self.conn
            .query_row(
                "SELECT * FROM executions WHERE id = ?1",
                params![_id],
                map_execution_row,
            )
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => {
                    StillrunError::not_found(format!("execution #{_id}"))
                }
                other => StillrunError::from(other),
            })
    }

    pub fn search_history(&self, filter: &HistoryFilter) -> Result<Vec<ExecutionRecord>> {
        let sql = history_sql(filter);
        let values = history_values(filter);
        let params = params_from_iter(values.iter().map(|value| value as &dyn ToSql));
        let mut statement = self.conn.prepare(&sql)?;
        let rows = statement.query_map(params, map_execution_row)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StillrunError::from)
    }

    pub fn upsert_job(&self, record: &JobRecord) -> Result<()> {
        let argv = redact::redact_argv(&record.argv);
        let argv_json = serde_json::to_string(&argv)?;
        self.conn.execute(
            r#"
            INSERT INTO jobs (
                id, name, label, argv_json, command, cwd, git_repo, git_branch, created_at_ms,
                updated_at_ms, status, pid, restart_count, stdout_path, stderr_path,
                plist_path, keep_alive, last_exit_code, last_cpu_percent, last_rss_kb
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                label = excluded.label,
                argv_json = excluded.argv_json,
                command = excluded.command,
                cwd = excluded.cwd,
                git_repo = excluded.git_repo,
                git_branch = excluded.git_branch,
                updated_at_ms = excluded.updated_at_ms,
                status = excluded.status,
                pid = excluded.pid,
                restart_count = excluded.restart_count,
                stdout_path = excluded.stdout_path,
                stderr_path = excluded.stderr_path,
                plist_path = excluded.plist_path,
                keep_alive = excluded.keep_alive,
                last_exit_code = excluded.last_exit_code,
                last_cpu_percent = excluded.last_cpu_percent,
                last_rss_kb = excluded.last_rss_kb
            "#,
            params![
                record.id,
                record.name,
                record.label,
                argv_json,
                record.command,
                record.cwd.to_string_lossy().to_string(),
                record
                    .git_repo
                    .as_ref()
                    .map(|path| path.to_string_lossy().to_string()),
                record.git_branch,
                record.created_at_ms,
                record.updated_at_ms,
                record.status.as_str(),
                record.pid.map(i64::from),
                record.restart_count,
                record.stdout_path.to_string_lossy().to_string(),
                record.stderr_path.to_string_lossy().to_string(),
                record.plist_path.to_string_lossy().to_string(),
                if record.keep_alive { 1 } else { 0 },
                record.last_exit_code,
                record.last_cpu_percent.map(f64::from),
                record.last_rss_kb.map(|rss| rss as i64),
            ],
        )?;
        Ok(())
    }

    pub fn update_job_runtime(&self, id: &str, update: &JobRuntimeUpdate) -> Result<JobRecord> {
        self.conn.execute(
            r#"
            UPDATE jobs
            SET status = ?2,
                pid = ?3,
                last_exit_code = ?4,
                last_cpu_percent = ?5,
                last_rss_kb = ?6,
                restart_count = CASE
                    WHEN ?7 IS NULL THEN restart_count
                    ELSE max(restart_count, ?7)
                END,
                updated_at_ms = ?8
            WHERE id = ?1 OR name = ?1 OR label = ?1
            "#,
            params![
                id,
                update.status.as_str(),
                update.pid.map(i64::from),
                update.last_exit_code,
                update.cpu_percent.map(f64::from),
                update.rss_kb.map(|rss| rss as i64),
                update.restart_count,
                update.updated_at_ms,
            ],
        )?;
        self.find_job(id)
    }

    pub fn list_jobs(&self) -> Result<Vec<JobRecord>> {
        let mut statement = self
            .conn
            .prepare("SELECT * FROM jobs ORDER BY created_at_ms DESC")?;
        let rows = statement.query_map([], map_job_row)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StillrunError::from)
    }

    pub fn find_job(&self, target: &str) -> Result<JobRecord> {
        self.conn
            .query_row(
                "SELECT * FROM jobs WHERE id = ?1 OR name = ?1 OR label = ?1 ORDER BY created_at_ms DESC LIMIT 1",
                params![target],
                map_job_row,
            )
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => {
                    StillrunError::not_found(format!("job '{target}'"))
                }
                other => StillrunError::from(other),
            })
    }

    fn ensure_executions_column(&self, name: &str, column_type: &str) -> Result<()> {
        self.ensure_column("executions", name, column_type)
    }

    fn ensure_jobs_column(&self, name: &str, column_type: &str) -> Result<()> {
        self.ensure_column("jobs", name, column_type)
    }

    fn ensure_column(&self, table: &str, name: &str, column_type: &str) -> Result<()> {
        let mut statement = self.conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let columns = statement
            .query_map([], |row| row.get::<_, String>("name"))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if columns.iter().any(|column| column == name) {
            return Ok(());
        }
        self.conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {name} {column_type}"),
            [],
        )?;
        Ok(())
    }
}

fn prepare_execution(
    record: &NewExecution,
    command_override: Option<&str>,
) -> Result<PreparedExecution> {
    let argv = redact::redact_argv(&record.argv);
    let command_source = command_override
        .map(str::to_string)
        .unwrap_or_else(|| format_argv(&argv));
    Ok(PreparedExecution {
        command: redact::redact_inline_secrets(&command_source),
        argv_json: serde_json::to_string(&argv)?,
        env_json: serde_json::to_string(&record.context.env)?,
        stdout: redact::redact_inline_secrets(&record.stdout),
        stderr: redact::redact_inline_secrets(&record.stderr),
    })
}

pub fn format_argv(argv: &[String]) -> String {
    argv.iter()
        .map(|arg| {
            if arg.is_empty() || arg.chars().any(char::is_whitespace) {
                format!("'{}'", arg.replace('\'', r#"'\''"#))
            } else {
                arg.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn history_sql(filter: &HistoryFilter) -> String {
    let fts_prefix = if filter
        .query
        .as_deref()
        .and_then(normalized_fts_query)
        .is_some()
    {
        "SELECT e.* FROM executions e JOIN executions_fts ON executions_fts.rowid = e.id WHERE executions_fts MATCH ?"
    } else {
        "SELECT e.* FROM executions e WHERE 1 = 1"
    };
    let cwd_clause = filter.cwd.as_ref().map(|_| " AND e.cwd = ?").unwrap_or("");
    let repo_clause = filter
        .repo
        .as_ref()
        .map(|_| " AND e.git_repo = ?")
        .unwrap_or("");
    let status_clause = filter
        .status
        .as_ref()
        .map(|_| " AND e.status = ?")
        .unwrap_or("");
    let started_after_clause = filter
        .started_after_ms
        .as_ref()
        .map(|_| " AND e.started_at_ms >= ?")
        .unwrap_or("");
    let started_before_clause = filter
        .started_before_ms
        .as_ref()
        .map(|_| " AND e.started_at_ms <= ?")
        .unwrap_or("");
    format!(
        "{fts_prefix}{cwd_clause}{repo_clause}{status_clause}{started_after_clause}{started_before_clause} ORDER BY e.started_at_ms DESC LIMIT ?"
    )
}

fn history_values(filter: &HistoryFilter) -> Vec<Value> {
    let query_value = filter
        .query
        .as_deref()
        .and_then(normalized_fts_query)
        .map(Value::Text)
        .into_iter();
    let cwd_value = filter
        .cwd
        .as_ref()
        .map(|cwd| Value::Text(cwd.to_string_lossy().to_string()))
        .into_iter();
    let repo_value = filter
        .repo
        .as_ref()
        .map(|repo| Value::Text(repo.to_string_lossy().to_string()))
        .into_iter();
    let status_value = filter
        .status
        .as_ref()
        .map(|status| Value::Text(status.as_str().to_string()))
        .into_iter();
    let started_after_value = filter.started_after_ms.map(Value::Integer).into_iter();
    let started_before_value = filter.started_before_ms.map(Value::Integer).into_iter();
    query_value
        .chain(cwd_value)
        .chain(repo_value)
        .chain(status_value)
        .chain(started_after_value)
        .chain(started_before_value)
        .chain(std::iter::once(Value::Integer(
            limit_or_default(filter.limit) as i64,
        )))
        .collect()
}

fn normalized_fts_query(query: &str) -> Option<String> {
    let value = query
        .split_whitespace()
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ");
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn limit_or_default(limit: usize) -> usize {
    if limit == 0 {
        25
    } else {
        limit
    }
}

fn map_execution_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ExecutionRecord> {
    let argv_json: String = row.get("argv_json")?;
    let status_text: String = row.get("status")?;
    let cwd: String = row.get("cwd")?;
    let git_repo: Option<String> = row.get("git_repo")?;
    Ok(ExecutionRecord {
        id: row.get("id")?,
        command: row.get("command")?,
        argv: serde_json::from_str(&argv_json).map_err(to_sql_error)?,
        cwd: PathBuf::from(cwd),
        git_repo: git_repo.map(PathBuf::from),
        git_branch: row.get("git_branch")?,
        started_at_ms: row.get("started_at_ms")?,
        ended_at_ms: row.get("ended_at_ms")?,
        duration_ms: row.get("duration_ms")?,
        exit_code: row.get("exit_code")?,
        status: ExecutionStatus::parse(&status_text).map_err(to_sql_error)?,
        env_json: row.get("env_json")?,
        stdout: row.get("stdout")?,
        stderr: row.get("stderr")?,
        pid: row.get::<_, Option<i64>>("pid")?.map(|pid| pid as u32),
        background_job_id: row.get("background_job_id")?,
        restart_count: row.get("restart_count")?,
        source: row.get("source")?,
        source_id: row.get("source_id")?,
    })
}

fn map_job_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<JobRecord> {
    let argv_json: String = row.get("argv_json")?;
    let status_text: String = row.get("status")?;
    let cwd: String = row.get("cwd")?;
    let git_repo: Option<String> = row.get("git_repo")?;
    let stdout_path: String = row.get("stdout_path")?;
    let stderr_path: String = row.get("stderr_path")?;
    let plist_path: String = row.get("plist_path")?;
    let keep_alive: i64 = row.get("keep_alive")?;
    Ok(JobRecord {
        id: row.get("id")?,
        name: row.get("name")?,
        label: row.get("label")?,
        argv: serde_json::from_str(&argv_json).map_err(to_sql_error)?,
        command: row.get("command")?,
        cwd: PathBuf::from(cwd),
        git_repo: git_repo.map(PathBuf::from),
        git_branch: row.get("git_branch")?,
        created_at_ms: row.get("created_at_ms")?,
        updated_at_ms: row.get("updated_at_ms")?,
        status: JobStatus::parse(&status_text).map_err(to_sql_error)?,
        pid: row.get::<_, Option<i64>>("pid")?.map(|pid| pid as u32),
        restart_count: row.get("restart_count")?,
        stdout_path: PathBuf::from(stdout_path),
        stderr_path: PathBuf::from(stderr_path),
        plist_path: PathBuf::from(plist_path),
        keep_alive: keep_alive != 0,
        last_exit_code: row.get("last_exit_code")?,
        last_cpu_percent: row
            .get::<_, Option<f64>>("last_cpu_percent")?
            .map(|v| v as f32),
        last_rss_kb: row
            .get::<_, Option<i64>>("last_rss_kb")?
            .map(|rss| rss as u64),
    })
}

fn to_sql_error(error: impl std::error::Error + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(error))
}
