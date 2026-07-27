use stillrun::redact::{
    argv_contains_sensitive_value, redact_argv, redact_env_value, redact_inline_secrets,
    RedactionPolicy, REDACTED,
};

#[test]
fn redacts_sensitive_environment_keys() {
    assert_eq!(
        redact_env_value(
            "OPENAI_API_KEY",
            "sk-live-secret",
            &RedactionPolicy::default()
        ),
        REDACTED
    );
}

#[test]
fn keeps_non_sensitive_environment_values() {
    assert_eq!(
        redact_env_value("RUST_LOG", "stillrun=debug", &RedactionPolicy::default()),
        "stillrun=debug"
    );
}

#[test]
fn redacts_inline_secret_assignments_in_commands() {
    let command = "curl -H 'Authorization: Bearer abc123' https://example.test?token=secret";
    let redacted = redact_inline_secrets(command, &RedactionPolicy::default());

    assert!(!redacted.contains("abc123"));
    assert!(!redacted.contains("secret"));
    assert!(redacted.contains("Authorization: Bearer"));
    assert!(redacted.contains("token="));
}

#[test]
fn redacts_sensitive_argv_values_without_changing_command_shape() {
    let argv = vec![
        "curl".to_string(),
        "--token".to_string(),
        "secret-token".to_string(),
        "--password=hunter2".to_string(),
        "safe=value".to_string(),
    ];

    let redacted = redact_argv(&argv, &RedactionPolicy::default());

    assert_eq!(
        redacted,
        vec![
            "curl".to_string(),
            "--token".to_string(),
            REDACTED.to_string(),
            format!("--password={REDACTED}"),
            "safe=value".to_string(),
        ]
    );
}

#[test]
fn configurable_redact_keys_apply_to_env_inline_text_and_argv() {
    let policy = RedactionPolicy::from_keys(["customsecret"]);

    assert_eq!(
        redact_env_value("CUSTOMSECRET", "env-value", &policy),
        REDACTED
    );
    assert_eq!(
        redact_inline_secrets("tool --customsecret sentinel", &policy),
        format!("tool --customsecret {REDACTED}")
    );
    assert_eq!(
        redact_argv(
            &[
                "tool".to_string(),
                "--customsecret".to_string(),
                "sentinel".to_string(),
                "--customsecret=another".to_string(),
            ],
            &policy,
        ),
        vec![
            "tool".to_string(),
            "--customsecret".to_string(),
            REDACTED.to_string(),
            format!("--customsecret={REDACTED}"),
        ]
    );
}

#[test]
fn detects_sensitive_argv_values_before_persisting_launchd_jobs() {
    let policy = RedactionPolicy::from_keys(["customsecret"]);

    assert!(argv_contains_sensitive_value(
        &[
            "curl".to_string(),
            "--customsecret".to_string(),
            "sentinel".to_string(),
        ],
        &policy,
    ));
    assert!(argv_contains_sensitive_value(
        &["curl".to_string(), "--customsecret=sentinel".to_string()],
        &policy,
    ));
    assert!(!argv_contains_sensitive_value(
        &["curl".to_string(), "https://example.test".to_string()],
        &policy,
    ));
}
