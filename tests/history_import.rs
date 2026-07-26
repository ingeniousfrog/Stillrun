use std::{fs, path::PathBuf};

use stillrun::{
    db::{ExecutionStatus, HistoryFilter, Store},
    history_import::{
        decode_history_bytes, decode_shell_history_bytes, format_import_progress,
        import_shell_history_file, import_shell_history_file_with_progress, parse_fish_history,
        parse_zsh_history, preview_shell_history_file, ImportProgressReporter,
        ImportProgressSnapshot, ShellKind,
    },
};

#[test]
fn parses_zsh_extended_history_and_plain_lines() {
    let entries = parse_zsh_history(
        r#": 1700000000:0;npm run dev
cargo test
: 1700000001:2;echo "hello stillrun"
"#,
    );

    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].command, "npm run dev");
    assert_eq!(entries[0].started_at_ms, Some(1_700_000_000_000));
    assert_eq!(entries[1].command, "cargo test");
    assert_eq!(entries[1].started_at_ms, None);
    assert_eq!(entries[2].command, r#"echo "hello stillrun""#);
}

#[test]
fn parses_fish_history_commands_with_timestamps() {
    let entries = parse_fish_history(
        r#"- cmd: cargo run -- history
  when: 1700000100
- cmd: npm run dev
  paths:
    - /tmp/project
"#,
    );

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].command, "cargo run -- history");
    assert_eq!(entries[0].started_at_ms, Some(1_700_000_100_000));
    assert_eq!(entries[1].command, "npm run dev");
    assert_eq!(entries[1].started_at_ms, None);
}

#[test]
fn imports_shell_history_as_searchable_imported_executions() {
    let temp = tempfile::tempdir().unwrap();
    let history_path = temp.path().join(".zsh_history");
    fs::write(
        &history_path,
        r#": 1700000000:0;npm run dev
python scripts/prompt.py
"#,
    )
    .unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();
    let home = PathBuf::from("/Users/tester");

    let first = import_shell_history_file(&store, &history_path, ShellKind::Zsh, &home).unwrap();
    let second = import_shell_history_file(&store, &history_path, ShellKind::Zsh, &home).unwrap();
    let matches = store
        .search_history(&HistoryFilter {
            query: Some("prompt".into()),
            status: Some(ExecutionStatus::Imported),
            limit: 10,
            ..HistoryFilter::default()
        })
        .unwrap();

    assert_eq!(first.imported, 2);
    assert_eq!(first.skipped, 0);
    assert_eq!(second.imported, 0);
    assert_eq!(second.skipped, 2);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].status, ExecutionStatus::Imported);
    assert_eq!(matches[0].cwd, home);
    assert!(matches[0].source.starts_with("shell:zsh:"));
}

#[test]
fn previews_shell_history_import_without_writing() {
    let temp = tempfile::tempdir().unwrap();
    let history_path = temp.path().join(".zsh_history");
    fs::write(
        &history_path,
        r#": 1700000000:0;npm run dev
: 1700000001:0;
python scripts/prompt.py
"#,
    )
    .unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();
    let home = PathBuf::from("/Users/tester");

    let preview = preview_shell_history_file(&store, &history_path, ShellKind::Zsh, &home).unwrap();
    let matches = store
        .search_history(&HistoryFilter {
            status: Some(ExecutionStatus::Imported),
            limit: 10,
            ..HistoryFilter::default()
        })
        .unwrap();

    assert_eq!(preview.scanned, 3);
    assert_eq!(preview.would_import, 2);
    assert_eq!(preview.empty, 1);
    assert_eq!(preview.files.len(), 1);
    assert_eq!(preview.files[0].path, history_path);
    assert!(matches.is_empty());
}

#[test]
fn preview_counts_existing_source_ids_as_duplicates() {
    let temp = tempfile::tempdir().unwrap();
    let history_path = temp.path().join(".zsh_history");
    fs::write(
        &history_path,
        r#": 1700000000:0;npm run dev
python scripts/prompt.py
"#,
    )
    .unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();
    let home = PathBuf::from("/Users/tester");

    import_shell_history_file(&store, &history_path, ShellKind::Zsh, &home).unwrap();
    let preview = preview_shell_history_file(&store, &history_path, ShellKind::Zsh, &home).unwrap();

    assert_eq!(preview.scanned, 2);
    assert_eq!(preview.duplicates, 2);
    assert_eq!(preview.would_import, 0);
}

#[test]
fn imports_non_utf8_shell_history_lossily() {
    let temp = tempfile::tempdir().unwrap();
    let history_path = temp.path().join(".zsh_history");
    fs::write(&history_path, b": 1700000000:0;printf 'bad-\xff-byte'\n").unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();
    let home = PathBuf::from("/Users/tester");

    let summary = import_shell_history_file(&store, &history_path, ShellKind::Zsh, &home).unwrap();
    let matches = store
        .search_history(&HistoryFilter {
            query: Some("bad".into()),
            status: Some(ExecutionStatus::Imported),
            limit: 10,
            ..HistoryFilter::default()
        })
        .unwrap();

    assert_eq!(summary.imported, 1);
    assert_eq!(matches.len(), 1);
    assert!(matches[0].command.contains("bad-"));
}

#[test]
fn decodes_gb18030_chinese_history_without_replacement_glyphs() {
    let bytes = [
        0x3a, 0x20, 0x31, 0x37, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x3a, 0x30, 0x3b, 0xc4,
        0xe3, 0xba, 0xc3, 0x20, 0xca, 0xc0, 0xbd, 0xe7, 0x0a,
    ];

    let decoded = decode_history_bytes(&bytes);

    assert!(decoded.contains("你好 世界"));
    assert!(!decoded.contains('\u{fffd}'));
}

#[test]
fn decodes_mixed_utf8_and_gb18030_history_by_line() {
    let mut bytes = "echo UTF8-中文\n".as_bytes().to_vec();
    bytes.extend_from_slice(&[
        0x3a, 0x20, 0x31, 0x37, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x3a, 0x30, 0x3b, 0xc4,
        0xe3, 0xba, 0xc3, 0x20, 0xca, 0xc0, 0xbd, 0xe7, 0x0a,
    ]);

    let decoded = decode_history_bytes(&bytes);

    assert!(decoded.contains("UTF8-中文"));
    assert!(decoded.contains("你好 世界"));
    assert!(!decoded.contains("涓"));
}

#[test]
fn decodes_zsh_metafied_utf8_history_before_encoding_detection() {
    let bytes = b": 1700000000:0;echo \"\xe5\xb0\x83\xa6\xe8\x83\xa3\x83\xac\xe6\x83\xb9\xaf\"\n";

    let decoded = decode_shell_history_bytes(bytes, ShellKind::Zsh);

    assert!(decoded.contains("将背景"));
    assert!(!decoded.contains("胣"));
    assert!(!decoded.contains('\u{fffd}'));
}

#[test]
fn imports_zsh_metafied_utf8_history_as_readable_chinese() {
    let temp = tempfile::tempdir().unwrap();
    let history_path = temp.path().join(".zsh_history");
    fs::write(
        &history_path,
        b": 1700000000:0;echo \"\xe5\xb0\x83\xa6\xe8\x83\xa3\x83\xac\xe6\x83\xb9\xaf\"\n",
    )
    .unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();
    let home = PathBuf::from("/Users/tester");

    let summary = import_shell_history_file(&store, &history_path, ShellKind::Zsh, &home).unwrap();
    let matches = store
        .search_history(&HistoryFilter {
            status: Some(ExecutionStatus::Imported),
            limit: 10,
            ..HistoryFilter::default()
        })
        .unwrap();

    assert_eq!(summary.imported, 1);
    assert_eq!(matches.len(), 1);
    assert!(matches[0].command.contains("将背景"));
    assert!(!matches[0].command.contains("胣"));
    assert!(!matches[0].command.contains('\u{fffd}'));
}

#[test]
fn reimport_refreshes_existing_imported_command_text() {
    let temp = tempfile::tempdir().unwrap();
    let history_path = temp.path().join(".zsh_history");
    fs::write(&history_path, b": 1700000000:0;printf 'bad-\xff-byte'\n").unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();
    let home = PathBuf::from("/Users/tester");

    let first = import_shell_history_file(&store, &history_path, ShellKind::Zsh, &home).unwrap();
    fs::write(&history_path, ": 1700000000:0;printf 'fixed-中文'\n").unwrap();
    let second = import_shell_history_file(&store, &history_path, ShellKind::Zsh, &home).unwrap();
    let matches = store
        .search_history(&HistoryFilter {
            query: Some("fixed".into()),
            status: Some(ExecutionStatus::Imported),
            limit: 10,
            ..HistoryFilter::default()
        })
        .unwrap();

    assert_eq!(first.imported, 1);
    assert_eq!(second.imported, 0);
    assert_eq!(second.skipped, 1);
    assert_eq!(matches.len(), 1);
    assert!(matches[0].command.contains("fixed-中文"));
    assert!(!matches[0].command.contains('\u{fffd}'));
}

#[test]
fn formats_import_progress_with_bar_and_counts() {
    let progress = format_import_progress(&ImportProgressSnapshot {
        kind: ShellKind::Zsh,
        path: PathBuf::from("/Users/tester/.zsh_history"),
        processed: 5,
        total: 10,
        imported: 4,
        skipped: 1,
        finished: false,
    });

    assert!(progress.contains("[==========----------]"));
    assert!(progress.contains("5/10"));
    assert!(progress.contains("imported=4"));
    assert!(progress.contains("skipped=1"));
    assert!(progress.contains(".zsh_history"));
}

#[test]
fn progress_reporter_receives_file_start_ticks_and_finish() {
    let temp = tempfile::tempdir().unwrap();
    let history_path = temp.path().join(".zsh_history");
    fs::write(
        &history_path,
        r#": 1700000000:0;echo one
: 1700000001:0;echo two
"#,
    )
    .unwrap();
    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();
    let home = PathBuf::from("/Users/tester");
    let mut reporter = RecordingReporter::default();

    let summary = import_shell_history_file_with_progress(
        &store,
        &history_path,
        ShellKind::Zsh,
        &home,
        &mut reporter,
    )
    .unwrap();

    assert_eq!(summary.imported, 2);
    assert_eq!(reporter.events.first().unwrap().processed, 0);
    assert_eq!(reporter.events.first().unwrap().total, 2);
    assert!(reporter.events.iter().any(|event| event.processed == 1));
    let last = reporter.events.last().unwrap();
    assert!(last.finished);
    assert_eq!(last.processed, 2);
    assert_eq!(last.imported, 2);
}

#[derive(Default)]
struct RecordingReporter {
    events: Vec<ImportProgressSnapshot>,
}

impl ImportProgressReporter for RecordingReporter {
    fn report(&mut self, snapshot: &ImportProgressSnapshot) {
        self.events.push(snapshot.clone());
    }
}
