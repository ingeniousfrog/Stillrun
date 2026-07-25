use std::{collections::BTreeMap, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LaunchdJobSpec {
    pub label: String,
    pub argv: Vec<String>,
    pub working_directory: PathBuf,
    pub environment: BTreeMap<String, String>,
    pub stdout_path: PathBuf,
    pub stderr_path: PathBuf,
    pub keep_alive: bool,
}

impl LaunchdJobSpec {
    pub fn to_plist_xml(&self) -> String {
        let argv_items = self
            .argv
            .iter()
            .map(|arg| format!("        <string>{}</string>", escape_xml(arg)))
            .collect::<Vec<_>>()
            .join("\n");
        let keep_alive = if self.keep_alive {
            "<true/>"
        } else {
            "<false/>"
        };
        let environment = if self.environment.is_empty() {
            String::new()
        } else {
            format!(
                r#"    <key>EnvironmentVariables</key>
    <dict>
{}
    </dict>
"#,
                self.environment
                    .iter()
                    .map(|(key, value)| {
                        format!(
                            "        <key>{}</key>\n        <string>{}</string>",
                            escape_xml(key),
                            escape_xml(value)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        };
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>ProgramArguments</key>
    <array>
{}
    </array>
    <key>WorkingDirectory</key>
    <string>{}</string>
    <key>StandardOutPath</key>
    <string>{}</string>
    <key>StandardErrorPath</key>
    <string>{}</string>
{}    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    {}
    <key>ProcessType</key>
    <string>Background</string>
</dict>
</plist>
"#,
            escape_xml(&self.label),
            argv_items,
            escape_xml(&self.working_directory.to_string_lossy()),
            escape_xml(&self.stdout_path.to_string_lossy()),
            escape_xml(&self.stderr_path.to_string_lossy()),
            environment,
            keep_alive
        )
    }
}

fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
