use std::{
    fs,
    path::{Path, PathBuf},
    process::Command as StdCommand,
    time::Duration,
};

use assert_cmd::Command;
use predicates::str::contains;
use stillrun::db::Store;

#[cfg(target_os = "macos")]
#[test]
fn launchd_background_job_lifecycle_e2e() {
    if std::env::var("STILLRUN_RUN_LAUNCHD_E2E").as_deref() != Ok("1") {
        eprintln!("skipped: set STILLRUN_RUN_LAUNCHD_E2E=1 to run real launchd E2E");
        return;
    }

    let temp = tempfile::tempdir().unwrap();
    let home = std::env::var("HOME").unwrap();
    let marker_path = temp.path().join("launchd-e2e.marker");
    let job_name = format!("stillrun-e2e-{}", std::process::id());
    let script = format!(
        "printf launchd-e2e > {}; sleep 30",
        shell_quote(&marker_path.to_string_lossy())
    );

    stillrun_command(&home, temp.path())
        .args([
            "run",
            "--background",
            "--name",
            &job_name,
            "--",
            "/bin/sh",
            "-c",
            &script,
        ])
        .assert()
        .success();

    let store = Store::open(temp.path().join("stillrun.db")).unwrap();
    store.initialize().unwrap();
    let job = store.find_job(&job_name).unwrap();
    let _cleanup = LaunchdCleanup {
        plist_path: job.plist_path.clone(),
    };

    wait_for_marker(&marker_path);
    assert_eq!(fs::read_to_string(&marker_path).unwrap(), "launchd-e2e");

    stillrun_command(&home, temp.path())
        .args(["status", &job_name])
        .assert()
        .success()
        .stdout(contains("Status:"))
        .stdout(contains("Recent stdout:"));

    stillrun_command(&home, temp.path())
        .args(["stop", &job_name])
        .assert()
        .success()
        .stdout(contains(&job.id));

    stillrun_command(&home, temp.path())
        .args(["jobs", "delete", &job_name])
        .assert()
        .success()
        .stdout(contains("deleted job"));

    assert!(!job.plist_path.exists());
}

#[cfg(target_os = "macos")]
fn stillrun_command(home: &str, stillrun_home: &Path) -> Command {
    let mut command = Command::cargo_bin("stillrun").unwrap();
    command
        .env_clear()
        .env("HOME", home)
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("STILLRUN_HOME", stillrun_home);
    command
}

#[cfg(not(target_os = "macos"))]
#[test]
fn launchd_background_job_lifecycle_e2e() {
    eprintln!("skipped: launchd E2E only runs on macOS");
}

#[cfg(target_os = "macos")]
struct LaunchdCleanup {
    plist_path: PathBuf,
}

#[cfg(target_os = "macos")]
impl Drop for LaunchdCleanup {
    fn drop(&mut self) {
        if let Some(domain) = launchd_domain() {
            let _ = StdCommand::new("launchctl")
                .args(["bootout", &domain])
                .arg(&self.plist_path)
                .output();
        }
        let _ = fs::remove_file(&self.plist_path);
    }
}

#[cfg(target_os = "macos")]
fn wait_for_marker(path: &Path) {
    for _ in 0..40 {
        if path.exists() {
            return;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    panic!(
        "launchd job did not write marker file at {}",
        path.display()
    );
}

#[cfg(target_os = "macos")]
fn launchd_domain() -> Option<String> {
    let output = StdCommand::new("id").arg("-u").output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(format!(
        "gui/{}",
        String::from_utf8_lossy(&output.stdout).trim()
    ))
}

#[cfg(target_os = "macos")]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r#"'\''"#))
}
