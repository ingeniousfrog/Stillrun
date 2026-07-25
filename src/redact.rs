pub const REDACTED: &str = "[redacted]";

pub fn is_sensitive_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    [
        "token",
        "secret",
        "password",
        "passwd",
        "api_key",
        "apikey",
        "credential",
        "private_key",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

pub fn redact_env_value(key: &str, value: &str) -> String {
    if is_sensitive_key(key) {
        REDACTED.to_string()
    } else {
        redact_inline_secrets(value)
    }
}

pub fn redact_inline_secrets(input: &str) -> String {
    let bearer_redacted =
        regex::Regex::new(r"(?i)(authorization:\s*bearer\s+)([A-Za-z0-9._~+/\-=]+)")
            .expect("valid bearer redaction regex")
            .replace_all(input, format!("${{1}}{REDACTED}"));

    let assignment_redacted = regex::Regex::new(
        r#"(?i)\b(token|secret|password|passwd|api_key|apikey|credential|private_key)=([^&\s'"]+)"#,
    )
    .expect("valid assignment redaction regex")
    .replace_all(&bearer_redacted, format!("${{1}}={REDACTED}"));

    regex::Regex::new(
        r#"(?i)(--(?:token|secret|password|passwd|api-key|api_key|apikey|credential|private-key|private_key)(?:=|\s+))([^ \t\n\r'"]+)"#,
    )
    .expect("valid flag redaction regex")
    .replace_all(&assignment_redacted, format!("${{1}}{REDACTED}"))
    .to_string()
}

pub fn redact_argv(argv: &[String]) -> Vec<String> {
    let mut redact_next = false;
    argv.iter()
        .map(|arg| {
            if redact_next {
                redact_next = false;
                return REDACTED.to_string();
            }

            if is_sensitive_flag(arg) {
                redact_next = true;
                return arg.clone();
            }

            redact_arg_assignment(arg)
        })
        .collect()
}

fn is_sensitive_flag(arg: &str) -> bool {
    let Some(flag) = arg.strip_prefix("--") else {
        return false;
    };
    if flag.contains('=') {
        return false;
    }
    is_sensitive_key(&flag.replace('-', "_"))
}

fn redact_arg_assignment(arg: &str) -> String {
    let Some((key, _value)) = arg.split_once('=') else {
        return redact_inline_secrets(arg);
    };
    let normalized_key = key.trim_start_matches('-').replace('-', "_");
    if is_sensitive_key(&normalized_key) {
        format!("{key}={REDACTED}")
    } else {
        redact_inline_secrets(arg)
    }
}
