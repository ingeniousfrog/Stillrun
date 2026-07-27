use std::collections::BTreeSet;

pub const REDACTED: &str = "[redacted]";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactionPolicy {
    sensitive_keys: BTreeSet<String>,
    sensitive_values: BTreeSet<String>,
}

impl Default for RedactionPolicy {
    fn default() -> Self {
        Self::from_keys(default_sensitive_keys())
    }
}

impl RedactionPolicy {
    pub fn from_keys<I, S>(keys: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self {
            sensitive_keys: keys
                .into_iter()
                .map(|key| normalize_key(key.as_ref()))
                .filter(|key| !key.is_empty())
                .collect(),
            sensitive_values: BTreeSet::new(),
        }
    }

    pub fn with_sensitive_values<I, S>(&self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut next = self.clone();
        next.sensitive_values.extend(
            values
                .into_iter()
                .map(|value| value.as_ref().to_string())
                .filter(|value| value.len() >= 3),
        );
        next
    }

    pub fn with_env_values_from_current_env(&self) -> Self {
        let values = std::env::vars()
            .filter_map(|(key, value)| self.is_sensitive_key(&key).then_some(value));
        self.with_sensitive_values(values)
    }

    pub fn is_sensitive_key(&self, key: &str) -> bool {
        let lower = normalize_key(key);
        self.sensitive_keys
            .iter()
            .any(|needle| lower.contains(needle))
    }

    fn key_regex_alternation(&self) -> String {
        let mut keys = self.sensitive_keys.iter().collect::<Vec<_>>();
        keys.sort_by_key(|key| std::cmp::Reverse(key.len()));
        keys.into_iter()
            .map(|key| regex::escape(key).replace('_', "[-_]"))
            .collect::<Vec<_>>()
            .join("|")
    }
}

pub fn default_sensitive_keys() -> [&'static str; 8] {
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
}

pub fn is_sensitive_key(key: &str) -> bool {
    RedactionPolicy::default().is_sensitive_key(key)
}

pub fn redact_env_value(key: &str, value: &str, policy: &RedactionPolicy) -> String {
    if policy.is_sensitive_key(key) {
        REDACTED.to_string()
    } else {
        redact_inline_secrets(value, policy)
    }
}

pub fn redact_inline_secrets(input: &str, policy: &RedactionPolicy) -> String {
    let bearer_redacted =
        regex::Regex::new(r"(?i)(authorization:\s*bearer\s+)([A-Za-z0-9._~+/\-=]+)")
            .expect("valid bearer redaction regex")
            .replace_all(input, format!("${{1}}{REDACTED}"));

    let key_pattern = policy.key_regex_alternation();
    let assignment_redacted = if key_pattern.is_empty() {
        bearer_redacted.to_string()
    } else {
        regex::Regex::new(&format!(r#"(?i)\b({key_pattern})=([^&\s'"]+)"#))
            .expect("valid assignment redaction regex")
            .replace_all(&bearer_redacted, format!("${{1}}={REDACTED}"))
            .to_string()
    };

    let flag_redacted = if key_pattern.is_empty() {
        assignment_redacted
    } else {
        regex::Regex::new(&format!(
            r#"(?i)(--(?:{key_pattern})(?:=|\s+))([^ \t\n\r'"]+)"#
        ))
        .expect("valid flag redaction regex")
        .replace_all(&assignment_redacted, format!("${{1}}{REDACTED}"))
        .to_string()
    };

    redact_sensitive_values(&flag_redacted, policy)
}

pub fn redact_argv(argv: &[String], policy: &RedactionPolicy) -> Vec<String> {
    let mut redact_next = false;
    argv.iter()
        .map(|arg| {
            if redact_next {
                redact_next = false;
                return REDACTED.to_string();
            }

            if is_sensitive_flag(arg, policy) {
                redact_next = true;
                return arg.clone();
            }

            redact_arg_assignment(arg, policy)
        })
        .collect()
}

pub fn argv_contains_sensitive_value(argv: &[String], policy: &RedactionPolicy) -> bool {
    let mut sensitive_flag = false;
    for arg in argv {
        if sensitive_flag {
            return true;
        }
        if is_sensitive_flag(arg, policy) {
            sensitive_flag = true;
            continue;
        }
        if arg.split_once('=').is_some_and(|(key, value)| {
            policy.is_sensitive_key(&key.trim_start_matches('-').replace('-', "_"))
                && value != REDACTED
        }) {
            return true;
        }
        if redact_inline_secrets(arg, policy) != *arg {
            return true;
        }
    }
    false
}

fn is_sensitive_flag(arg: &str, policy: &RedactionPolicy) -> bool {
    let Some(flag) = arg.strip_prefix("--") else {
        return false;
    };
    if flag.contains('=') {
        return false;
    }
    policy.is_sensitive_key(&flag.replace('-', "_"))
}

fn redact_arg_assignment(arg: &str, policy: &RedactionPolicy) -> String {
    let Some((key, _value)) = arg.split_once('=') else {
        return redact_inline_secrets(arg, policy);
    };
    let normalized_key = key.trim_start_matches('-').replace('-', "_");
    if policy.is_sensitive_key(&normalized_key) {
        format!("{key}={REDACTED}")
    } else {
        redact_inline_secrets(arg, policy)
    }
}

fn redact_sensitive_values(input: &str, policy: &RedactionPolicy) -> String {
    policy
        .sensitive_values
        .iter()
        .fold(input.to_string(), |text, value| {
            text.replace(value, REDACTED)
        })
}

fn normalize_key(key: &str) -> String {
    key.trim()
        .trim_start_matches('-')
        .replace('-', "_")
        .to_ascii_lowercase()
}
