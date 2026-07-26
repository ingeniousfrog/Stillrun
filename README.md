# Stillrun

[中文文档](README.zh-CN.md)

Stillrun is a macOS command lifecycle runtime. It records terminal commands,
lets you search and replay them, and can promote useful commands into
launchd-backed background Jobs with logs and status.

## Install

```sh
./scripts/install.sh
```

The installer runs `cargo install --path . --force`, then can import existing
shell history and install the shell hook when you approve the prompts.

For a non-interactive install:

```sh
cargo install --path .
```

## Quick Help

```sh
stillrun -h
stillrun <command> -h
```

`stillrun -h` shows the capability list and short examples. Command-level help
shows the flags for one workflow.

## Capabilities

| Capability | Commands | Example |
| --- | --- | --- |
| Run and record | `run` | `stillrun run -- npm run dev` |
| Search history | `history` | `stillrun history --query "npm" --sort oldest` |
| Import shell history | `import-history` | `stillrun import-history --shell auto --preview` |
| Replay safely | `replay` | `stillrun replay 12 --preview` |
| Promote to Job | `promote` | `stillrun promote 12 --name dev-server` |
| Manage Jobs | `jobs`, `status`, `start`, `stop`, `restart` | `stillrun status dev-server` |
| Read logs | `logs` | `stillrun logs dev-server --follow` |
| Inspect metadata | `inspect` | `stillrun inspect 12 --json` |
| Manage config | `config` | `stillrun config set max-output-bytes 2097152` |
| Shell hooks | `hook` | `stillrun hook install --shell auto` |
| Shell completion | `completion` | `stillrun completion zsh > ~/.stillrun-completion.zsh` |

## Examples

Run and record a foreground command:

```sh
stillrun run -- npm run dev
stillrun run -- zsh -lc "curl -s https://example.com | jq ."
```

Search and inspect history:

```sh
stillrun history --query "npm"
stillrun history --status success
stillrun history --status imported --query "cargo"
stillrun history --since-ms 1720000000000 --until-ms 1730000000000
stillrun history --sort oldest --full --details
stillrun inspect 1 --json
```

Import existing shell history with a preview:

```sh
stillrun import-history --shell auto --preview
stillrun import-history --shell auto
stillrun import-history --shell zsh --file ~/.zsh_history --yes
```

Replay a command:

```sh
stillrun replay 1 --preview
stillrun replay 1 --yes
```

Create and manage a background Job:

```sh
stillrun run --background --name dev-server -- npm run dev
stillrun run --background --keep-alive --name api-server -- cargo run
stillrun jobs
stillrun status dev-server
stillrun logs dev-server --follow
stillrun jobs monitor dev-server --once --cpu-alert 90 --rss-alert-mb 1024
stillrun stop dev-server
stillrun start dev-server
stillrun restart dev-server
stillrun jobs delete dev-server
```

Promote a previous execution into a Job:

```sh
stillrun run -- python scripts/download.py
stillrun history --query download.py
stillrun promote 1 --name downloader
stillrun logs downloader --follow
```

Manage config:

```sh
stillrun config show
stillrun config show --json
stillrun config path
stillrun config set max-output-bytes 2097152
stillrun config redact add session_token
stillrun config redact list
```

Install helpers for future commands:

```sh
stillrun hook install --shell auto
stillrun hook print --shell zsh
stillrun completion zsh > ~/.stillrun-completion.zsh
printf '\nsource ~/.stillrun-completion.zsh\n' >> ~/.zshrc
```

## Storage

Stillrun stores local state under:

```text
~/Library/Application Support/Stillrun/stillrun.db
~/Library/Application Support/Stillrun/logs/
~/Library/Application Support/Stillrun/config.toml
```

Background Job plists are written to `~/Library/LaunchAgents/` by default.

Use `STILLRUN_HOME` to isolate state during testing or development:

```sh
STILLRUN_HOME=/tmp/stillrun-dev cargo run -- history
```

## Security Notes

Stillrun redacts common sensitive environment keys and inline command/output
patterns before writing to SQLite. Examples include tokens, passwords, secrets,
API keys, `Authorization: Bearer ...`, `token=...`, `--token value`, and
`--password=value`.

Replay clears the current process environment before restoring recorded
non-redacted values. Imported shell-history replay asks for preview or
confirmation so old commands are not re-run accidentally.

## Testing

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```

Run the real macOS launchd lifecycle E2E only when you explicitly want to touch
the user launchd session:

```sh
STILLRUN_RUN_LAUNCHD_E2E=1 cargo test --test launchd_e2e -- --nocapture
```
