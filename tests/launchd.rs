use std::collections::BTreeMap;
use std::path::PathBuf;

use stillrun::jobs::launchd::LaunchdJobSpec;

#[test]
fn launchd_plist_contains_persistent_command_contract() {
    let spec = LaunchdJobSpec {
        label: "com.stillrun.dev-server".into(),
        argv: vec!["npm".into(), "run".into(), "dev".into()],
        working_directory: PathBuf::from("/Users/test/project"),
        environment: BTreeMap::new(),
        stdout_path: PathBuf::from("/tmp/stillrun/out.log"),
        stderr_path: PathBuf::from("/tmp/stillrun/err.log"),
        keep_alive: true,
    };

    let plist = spec.to_plist_xml();

    assert!(plist.contains("<key>Label</key>"));
    assert!(plist.contains("<string>com.stillrun.dev-server</string>"));
    assert!(plist.contains("<key>ProgramArguments</key>"));
    assert!(plist.contains("<string>npm</string>"));
    assert!(plist.contains("<string>dev</string>"));
    assert!(plist.contains("<key>WorkingDirectory</key>"));
    assert!(plist.contains("<key>KeepAlive</key>"));
    assert!(plist.contains("<true/>"));
}

#[test]
fn launchd_plist_escapes_xml_sensitive_values() {
    let spec = LaunchdJobSpec {
        label: "com.stillrun.escape".into(),
        argv: vec!["echo".into(), "a < b & c".into()],
        working_directory: PathBuf::from("/tmp/a&b"),
        environment: BTreeMap::new(),
        stdout_path: PathBuf::from("/tmp/out.log"),
        stderr_path: PathBuf::from("/tmp/err.log"),
        keep_alive: false,
    };

    let plist = spec.to_plist_xml();

    assert!(plist.contains("a &lt; b &amp; c"));
    assert!(plist.contains("/tmp/a&amp;b"));
}

#[test]
fn launchd_plist_includes_restorable_environment_variables() {
    let mut environment = BTreeMap::new();
    environment.insert("SAFE_FLAG".to_string(), "yes & ready".to_string());
    let spec = LaunchdJobSpec {
        label: "com.stillrun.env".into(),
        argv: vec!["env".into()],
        working_directory: PathBuf::from("/tmp"),
        environment,
        stdout_path: PathBuf::from("/tmp/out.log"),
        stderr_path: PathBuf::from("/tmp/err.log"),
        keep_alive: false,
    };

    let plist = spec.to_plist_xml();

    assert!(plist.contains("<key>EnvironmentVariables</key>"));
    assert!(plist.contains("<key>SAFE_FLAG</key>"));
    assert!(plist.contains("<string>yes &amp; ready</string>"));
}
