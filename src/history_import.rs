use std::{
    collections::BTreeMap,
    env, fs,
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
};

use clap::ValueEnum;
use encoding_rs::{GB18030, GBK};
use serde::{Deserialize, Serialize};

use crate::{
    context::CommandContext,
    db::{ExecutionStatus, NewExecution, Store},
    execution::now_ms,
    Result,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
pub enum ShellKind {
    Zsh,
    Bash,
    Fish,
}

impl ShellKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Zsh => "zsh",
            Self::Bash => "bash",
            Self::Fish => "fish",
        }
    }

    pub fn replay_argv(self, command: &str) -> Vec<String> {
        match self {
            Self::Zsh => vec!["zsh".into(), "-lc".into(), command.into()],
            Self::Bash => vec!["bash".into(), "-lc".into(), command.into()],
            Self::Fish => vec!["fish".into(), "-c".into(), command.into()],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
pub enum ImportShellSelection {
    Auto,
    Zsh,
    Bash,
    Fish,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellHistoryEntry {
    pub command: String,
    pub started_at_ms: Option<i64>,
    pub line_number: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportSummary {
    pub imported: usize,
    pub skipped: usize,
    pub scanned: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportPreview {
    pub files: Vec<ImportFilePreview>,
    pub scanned: usize,
    pub would_import: usize,
    pub duplicates: usize,
    pub empty: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportFilePreview {
    pub kind: ShellKind,
    pub path: PathBuf,
    pub scanned: usize,
    pub would_import: usize,
    pub duplicates: usize,
    pub empty: usize,
}

impl ImportPreview {
    fn push_file(&mut self, file: ImportFilePreview) {
        self.scanned += file.scanned;
        self.would_import += file.would_import;
        self.duplicates += file.duplicates;
        self.empty += file.empty;
        self.files.push(file);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportProgressSnapshot {
    pub kind: ShellKind,
    pub path: PathBuf,
    pub processed: usize,
    pub total: usize,
    pub imported: usize,
    pub skipped: usize,
    pub finished: bool,
}

pub trait ImportProgressReporter {
    fn report(&mut self, snapshot: &ImportProgressSnapshot);
}

#[derive(Debug, Default)]
pub struct NoopImportProgress;

impl ImportProgressReporter for NoopImportProgress {
    fn report(&mut self, _snapshot: &ImportProgressSnapshot) {}
}

#[derive(Debug)]
pub struct TerminalImportProgress {
    enabled: bool,
    last_len: usize,
}

impl TerminalImportProgress {
    pub fn stderr() -> Self {
        Self {
            enabled: io::stderr().is_terminal(),
            last_len: 0,
        }
    }
}

impl ImportProgressReporter for TerminalImportProgress {
    fn report(&mut self, snapshot: &ImportProgressSnapshot) {
        if !self.enabled || !should_render_progress(snapshot) {
            return;
        }

        let line = format_import_progress(snapshot);
        let padding = " ".repeat(self.last_len.saturating_sub(line.len()));
        let mut stderr = io::stderr();
        let _ = write!(stderr, "\r{line}{padding}");
        if snapshot.finished {
            let _ = writeln!(stderr);
            self.last_len = 0;
        } else {
            self.last_len = line.len();
        }
        let _ = stderr.flush();
    }
}

pub fn import_shell_history_auto(store: &Store) -> Result<ImportSummary> {
    let mut progress = NoopImportProgress;
    import_shell_history_auto_with_progress(store, &mut progress)
}

pub fn import_shell_history_auto_with_progress(
    store: &Store,
    progress: &mut dyn ImportProgressReporter,
) -> Result<ImportSummary> {
    let home = user_home();
    let mut total = ImportSummary::default();
    for (kind, path) in default_history_files(&home) {
        if !path.exists() {
            continue;
        }
        let summary = import_shell_history_file_with_progress(store, &path, kind, &home, progress)?;
        total.imported += summary.imported;
        total.skipped += summary.skipped;
        total.scanned += summary.scanned;
    }
    Ok(total)
}

pub fn import_selected_shell_history(
    store: &Store,
    selection: ImportShellSelection,
    file: Option<PathBuf>,
) -> Result<ImportSummary> {
    let mut progress = NoopImportProgress;
    import_selected_shell_history_with_progress(store, selection, file, &mut progress)
}

pub fn preview_selected_shell_history(
    store: &Store,
    selection: ImportShellSelection,
    file: Option<PathBuf>,
) -> Result<ImportPreview> {
    match (selection, file) {
        (ImportShellSelection::Auto, None) => preview_shell_history_auto(store),
        (ImportShellSelection::Auto, Some(path)) => {
            let kind = ShellKind::from_path_hint(&path).unwrap_or(ShellKind::Zsh);
            preview_shell_history_file(store, &path, kind, &user_home())
        }
        (ImportShellSelection::Zsh, file) => preview_shell_history_file(
            store,
            &history_file(file, ShellKind::Zsh),
            ShellKind::Zsh,
            &user_home(),
        ),
        (ImportShellSelection::Bash, file) => preview_shell_history_file(
            store,
            &history_file(file, ShellKind::Bash),
            ShellKind::Bash,
            &user_home(),
        ),
        (ImportShellSelection::Fish, file) => preview_shell_history_file(
            store,
            &history_file(file, ShellKind::Fish),
            ShellKind::Fish,
            &user_home(),
        ),
    }
}

pub fn preview_shell_history_auto(store: &Store) -> Result<ImportPreview> {
    let home = user_home();
    let mut preview = ImportPreview::default();
    for (kind, path) in default_history_files(&home) {
        if !path.exists() {
            continue;
        }
        preview.push_file(preview_shell_history_file_entry(store, &path, kind, &home)?);
    }
    Ok(preview)
}

pub fn preview_shell_history_file(
    store: &Store,
    path: &Path,
    kind: ShellKind,
    _fallback_cwd: &Path,
) -> Result<ImportPreview> {
    let mut preview = ImportPreview::default();
    preview.push_file(preview_shell_history_file_entry(
        store,
        path,
        kind,
        _fallback_cwd,
    )?);
    Ok(preview)
}

pub fn import_selected_shell_history_with_progress(
    store: &Store,
    selection: ImportShellSelection,
    file: Option<PathBuf>,
    progress: &mut dyn ImportProgressReporter,
) -> Result<ImportSummary> {
    match (selection, file) {
        (ImportShellSelection::Auto, None) => {
            import_shell_history_auto_with_progress(store, progress)
        }
        (ImportShellSelection::Auto, Some(path)) => {
            let kind = ShellKind::from_path_hint(&path).unwrap_or(ShellKind::Zsh);
            import_shell_history_file_with_progress(store, &path, kind, &user_home(), progress)
        }
        (ImportShellSelection::Zsh, file) => import_shell_history_file_with_progress(
            store,
            &history_file(file, ShellKind::Zsh),
            ShellKind::Zsh,
            &user_home(),
            progress,
        ),
        (ImportShellSelection::Bash, file) => import_shell_history_file_with_progress(
            store,
            &history_file(file, ShellKind::Bash),
            ShellKind::Bash,
            &user_home(),
            progress,
        ),
        (ImportShellSelection::Fish, file) => import_shell_history_file_with_progress(
            store,
            &history_file(file, ShellKind::Fish),
            ShellKind::Fish,
            &user_home(),
            progress,
        ),
    }
}

pub fn import_shell_history_file(
    store: &Store,
    path: &Path,
    kind: ShellKind,
    fallback_cwd: &Path,
) -> Result<ImportSummary> {
    let mut progress = NoopImportProgress;
    import_shell_history_file_with_progress(store, path, kind, fallback_cwd, &mut progress)
}

pub fn import_shell_history_file_with_progress(
    store: &Store,
    path: &Path,
    kind: ShellKind,
    fallback_cwd: &Path,
    progress: &mut dyn ImportProgressReporter,
) -> Result<ImportSummary> {
    let bytes = fs::read(path)?;
    let text = decode_shell_history_bytes(&bytes, kind);
    let source = shell_history_source(kind, path);
    let entries = match kind {
        ShellKind::Zsh => parse_zsh_history(&text),
        ShellKind::Bash => parse_bash_history(&text),
        ShellKind::Fish => parse_fish_history(&text),
    };
    let mut summary = ImportSummary {
        scanned: entries.len(),
        ..ImportSummary::default()
    };
    report_import_progress(progress, kind, path, &summary, 0, false);
    let fallback_started_at_ms = now_ms();
    for (index, entry) in entries.into_iter().enumerate() {
        let source_id = entry.line_number.to_string();
        let started_at_ms = entry
            .started_at_ms
            .unwrap_or(fallback_started_at_ms + entry.line_number as i64);
        let command = entry.command.trim();
        if command.is_empty() {
            summary.skipped += 1;
            continue;
        }
        let inserted = store.insert_imported_execution(
            &NewExecution {
                argv: kind.replay_argv(command),
                context: CommandContext {
                    cwd: fallback_cwd.to_path_buf(),
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
            &source,
            &source_id,
            command,
        )?;
        if inserted.is_some() {
            summary.imported += 1;
        } else {
            summary.skipped += 1;
        }
        report_import_progress(progress, kind, path, &summary, index + 1, false);
    }
    report_import_progress(progress, kind, path, &summary, summary.scanned, true);
    Ok(summary)
}

pub fn decode_shell_history_bytes(bytes: &[u8], kind: ShellKind) -> String {
    match kind {
        ShellKind::Zsh => decode_zsh_history_bytes(bytes),
        ShellKind::Bash | ShellKind::Fish => decode_history_bytes(bytes),
    }
}

pub fn decode_history_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_string();
    }

    let mut decoded = String::new();
    for chunk in bytes.split_inclusive(|byte| *byte == b'\n') {
        let (line, newline) = chunk
            .strip_suffix(b"\n")
            .map(|line| (line, "\n"))
            .unwrap_or((chunk, ""));
        decoded.push_str(&decode_history_line(line));
        decoded.push_str(newline);
    }
    decoded
}

fn decode_zsh_history_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_string();
    }

    let mut decoded = String::new();
    for chunk in bytes.split_inclusive(|byte| *byte == b'\n') {
        let (line, newline) = chunk
            .strip_suffix(b"\n")
            .map(|line| (line, "\n"))
            .unwrap_or((chunk, ""));
        if std::str::from_utf8(line).is_ok() {
            decoded.push_str(&decode_history_line(line));
        } else {
            let unmetafied = unmetafy_zsh_line(line);
            decoded.push_str(&decode_history_line(&unmetafied));
        }
        decoded.push_str(newline);
    }
    decoded
}

fn decode_history_line(bytes: &[u8]) -> String {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_string();
    }

    let candidates = [GB18030, GBK]
        .iter()
        .map(|encoding| {
            let (text, _encoding_used, had_errors) = encoding.decode(bytes);
            let text = text.into_owned();
            let replacement_count = text.matches('\u{fffd}').count();
            (text, had_errors, replacement_count)
        })
        .collect::<Vec<_>>();

    candidates
        .into_iter()
        .min_by_key(|(_text, had_errors, replacement_count)| (*had_errors, *replacement_count))
        .map(|(text, _had_errors, _replacement_count)| text)
        .unwrap_or_else(|| String::from_utf8_lossy(bytes).into_owned())
}

fn unmetafy_zsh_line(bytes: &[u8]) -> Vec<u8> {
    const ZSH_META: u8 = 0x83;
    let mut unmetafied = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == ZSH_META && index + 1 < bytes.len() {
            unmetafied.push(bytes[index + 1] ^ 0x20);
            index += 2;
        } else {
            unmetafied.push(bytes[index]);
            index += 1;
        }
    }
    unmetafied
}

pub fn format_import_progress(snapshot: &ImportProgressSnapshot) -> String {
    let file_name = match snapshot.path.file_name().and_then(|name| name.to_str()) {
        Some(name) => name.to_string(),
        None => snapshot.path.to_string_lossy().to_string(),
    };
    let phase = if snapshot.finished {
        "done"
    } else {
        "importing"
    };
    format!(
        "{phase} {} {} {}/{} imported={} skipped={} {}",
        snapshot.kind.as_str(),
        format_progress_bar(snapshot.processed, snapshot.total, 20),
        snapshot.processed,
        snapshot.total,
        snapshot.imported,
        snapshot.skipped,
        file_name
    )
}

pub fn format_import_preview(preview: &ImportPreview) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "Import preview: files={} scanned={} would_import={} duplicates={} empty={}\n",
        preview.files.len(),
        preview.scanned,
        preview.would_import,
        preview.duplicates,
        preview.empty
    ));
    for file in &preview.files {
        output.push_str(&format!(
            "  {} {} scanned={} would_import={} duplicates={} empty={}\n",
            file.kind.as_str(),
            file.path.display(),
            file.scanned,
            file.would_import,
            file.duplicates,
            file.empty
        ));
    }
    output
}

fn preview_shell_history_file_entry(
    store: &Store,
    path: &Path,
    kind: ShellKind,
    _fallback_cwd: &Path,
) -> Result<ImportFilePreview> {
    let bytes = fs::read(path)?;
    let text = decode_shell_history_bytes(&bytes, kind);
    let source = shell_history_source(kind, path);
    let entries = match kind {
        ShellKind::Zsh => parse_zsh_history(&text),
        ShellKind::Bash => parse_bash_history(&text),
        ShellKind::Fish => parse_fish_history(&text),
    };
    let mut file = ImportFilePreview {
        kind,
        path: path.to_path_buf(),
        scanned: entries.len(),
        would_import: 0,
        duplicates: 0,
        empty: 0,
    };
    for entry in entries {
        let command = entry.command.trim();
        if command.is_empty() {
            file.empty += 1;
            continue;
        }
        let source_id = entry.line_number.to_string();
        if store.execution_source_id_exists(&source, &source_id)? {
            file.duplicates += 1;
        } else {
            file.would_import += 1;
        }
    }
    Ok(file)
}

fn report_import_progress(
    progress: &mut dyn ImportProgressReporter,
    kind: ShellKind,
    path: &Path,
    summary: &ImportSummary,
    processed: usize,
    finished: bool,
) {
    progress.report(&ImportProgressSnapshot {
        kind,
        path: path.to_path_buf(),
        processed,
        total: summary.scanned,
        imported: summary.imported,
        skipped: summary.skipped,
        finished,
    });
}

fn should_render_progress(snapshot: &ImportProgressSnapshot) -> bool {
    snapshot.finished
        || snapshot.processed == 0
        || snapshot.processed == snapshot.total
        || snapshot.processed % 100 == 0
}

fn format_progress_bar(processed: usize, total: usize, width: usize) -> String {
    let filled = if total == 0 {
        width
    } else {
        processed.min(total) * width / total
    };
    format!(
        "[{}{}]",
        "=".repeat(filled),
        "-".repeat(width.saturating_sub(filled))
    )
}

pub fn parse_zsh_history(text: &str) -> Vec<ShellHistoryEntry> {
    text.lines()
        .enumerate()
        .filter_map(|(index, line)| parse_zsh_line(index + 1, line))
        .collect()
}

pub fn parse_bash_history(text: &str) -> Vec<ShellHistoryEntry> {
    text.lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let command = line.trim();
            if command.is_empty() {
                return None;
            }
            Some(ShellHistoryEntry {
                command: command.to_string(),
                started_at_ms: None,
                line_number: index + 1,
            })
        })
        .collect()
}

pub fn parse_fish_history(text: &str) -> Vec<ShellHistoryEntry> {
    let mut entries = Vec::new();
    let mut current: Option<ShellHistoryEntry> = None;
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if let Some(command) = trimmed.strip_prefix("- cmd: ") {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
            current = Some(ShellHistoryEntry {
                command: unquote_fish_value(command.trim()).to_string(),
                started_at_ms: None,
                line_number: index + 1,
            });
        } else if let Some(timestamp) = trimmed.strip_prefix("when: ") {
            if let Some(entry) = current.as_mut() {
                entry.started_at_ms = parse_seconds_timestamp(timestamp.trim());
            }
        }
    }
    if let Some(entry) = current {
        entries.push(entry);
    }
    entries
}

impl ShellKind {
    fn from_path_hint(path: &Path) -> Option<Self> {
        let text = path.to_string_lossy();
        if text.contains("fish_history") {
            Some(Self::Fish)
        } else if text.contains("bash_history") {
            Some(Self::Bash)
        } else if text.contains("zsh_history") {
            Some(Self::Zsh)
        } else {
            None
        }
    }
}

fn parse_zsh_line(line_number: usize, line: &str) -> Option<ShellHistoryEntry> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix(": ") {
        let (metadata, command) = rest.split_once(';')?;
        let timestamp = metadata.split(':').next()?.trim();
        return Some(ShellHistoryEntry {
            command: command.trim().to_string(),
            started_at_ms: parse_seconds_timestamp(timestamp),
            line_number,
        });
    }
    Some(ShellHistoryEntry {
        command: trimmed.to_string(),
        started_at_ms: None,
        line_number,
    })
}

fn parse_seconds_timestamp(value: &str) -> Option<i64> {
    value.parse::<i64>().ok().map(|seconds| seconds * 1_000)
}

fn unquote_fish_value(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
}

fn default_history_files(home: &Path) -> Vec<(ShellKind, PathBuf)> {
    vec![
        (
            ShellKind::Zsh,
            env::var_os("HISTFILE")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".zsh_history")),
        ),
        (ShellKind::Bash, home.join(".bash_history")),
        (ShellKind::Fish, home.join(".local/share/fish/fish_history")),
    ]
}

fn history_file(file: Option<PathBuf>, kind: ShellKind) -> PathBuf {
    if let Some(file) = file {
        return file;
    }
    let home = user_home();
    default_history_files(&home)
        .into_iter()
        .find_map(|(candidate, path)| (candidate == kind).then_some(path))
        .unwrap_or_else(|| home.join(".zsh_history"))
}

fn shell_history_source(kind: ShellKind, path: &Path) -> String {
    format!("shell:{}:{}", kind.as_str(), path.display())
}

fn user_home() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}
