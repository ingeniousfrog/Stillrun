use stillrun::redact::{redact_argv, redact_env_value, redact_inline_secrets, REDACTED};

#[test]
fn redacts_sensitive_environment_keys() {
    assert_eq!(
        redact_env_value("OPENAI_API_KEY", "sk-live-secret"),
        REDACTED
    );
}

#[test]
fn keeps_non_sensitive_environment_values() {
    assert_eq!(
        redact_env_value("RUST_LOG", "stillrun=debug"),
        "stillrun=debug"
    );
}

#[test]
fn redacts_inline_secret_assignments_in_commands() {
    let command = "curl -H 'Authorization: Bearer abc123' https://example.test?token=secret";
    let redacted = redact_inline_secrets(command);

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

    let redacted = redact_argv(&argv);

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
