# Stillrun

English | [简体中文](README.zh-CN.md)

macOS CLI for **command history**, **safe replay**, and **launchd-backed Jobs**.

---

## Overview

Stillrun turns terminal commands into searchable, inspectable records. Useful
commands can be promoted into background Jobs with logs, status, and resource
samples—without replacing your shell.

**Primary use cases:** recoverable command history, one-shot or long-running
background work, local Job observability.

**Out of scope:** TUI / Web UI, AI search, remote nodes, Linux / Windows Job
backends (MVP is macOS + launchd only).

Sensitive values are redacted before persistence. Replay restores recorded
non-sensitive environment only—not your current shell session.

---

## Features

| Area | Commands | Description |
| --- | --- | --- |
| Run & record | `run` | Capture argv, cwd, Git branch/head, timing, exit status, stdout/stderr, redacted env |
| Shell wrapper | `run --shell` | Wrap complex shell syntax such as pipes, redirects, aliases, and functions in your shell |
| History | `history` | Search with FTS5 + substring fallback; filter by cwd, repo, branch, exit code, status, time; output text or JSON |
| Import | `import-history` | Preview / import local zsh, bash, fish history |
| Replay | `replay` | Re-run from original cwd with recorded non-sensitive env |
| Promote | `promote` | Turn a past execution into a launchd Job |
| Jobs | `jobs`, `status`, `start`, `stop`, `restart` | Lifecycle, dashboard, process-tree samples, events, optional background monitor |
| Logs | `logs` | Tail or follow Job stdout/stderr |
| Inspect | `inspect` | Human or JSON view of an execution or Job |
| Config | `config` | Local TOML settings and redact-key management |
| Shell integration | `hook`, `completion` | Auto-record future shell commands; tab completion |

---

## Quick start

```bash
brew tap ingeniousfrog/tap
brew install stillrun
```

Verify:

```bash
stillrun -h
stillrun run -- printf 'hello stillrun\n'
stillrun run --shell 'npm run dev 2>&1 | tee dev.log'
stillrun history --query hello
```

After installing, you can optionally import existing shell history and install a
shell hook:

```bash
stillrun import-history --shell auto --preview
stillrun hook install --shell auto
```

---

## Workflow

```mermaid
flowchart LR
  subgraph capture [Capture]
    R[run]
    H[hook]
    I[import-history]
  end
  subgraph store [Store]
    DB[(SQLite + FTS5)]
  end
  subgraph reuse [Reuse]
    S[history / inspect]
    P[replay]
    J[promote / jobs]
  end
  R --> DB
  H --> DB
  I --> DB
  DB --> S
  DB --> P
  DB --> J
  J --> L[launchd + logs]
```

**Typical path:** record → search → replay or promote → manage Job lifecycle.

```mermaid
sequenceDiagram
  participant U as User
  participant S as stillrun
  participant L as launchd
  U->>S: run -- printf hello
  S-->>U: recorded execution #1
  U->>S: history --query hello
  U->>S: run --background --name demo -- sleep 30
  S->>L: bootstrap plist
  U->>S: status demo / logs demo
  U->>S: stop demo
```

---

## Installation

**Current version:** `0.1.0`

| Method | Command |
| --- | --- |
| Homebrew | `brew tap ingeniousfrog/tap && brew install stillrun` |
| Homebrew direct | `brew install ingeniousfrog/tap/stillrun` |
| Source interactive | `./scripts/install.sh` |
| Source non-interactive | `cargo install --path .` |
| Dev binary | `cargo run -- <args>` |

Requires Rust **1.78+** and macOS (background Jobs use launchd).

Isolate local state while experimenting:

```bash
export STILLRUN_HOME=/tmp/stillrun-dev
stillrun run -- printf 'isolated\n'
stillrun history
```

---

## Examples

```bash
# Record a foreground command
stillrun run -- printf 'hello stillrun\n'
stillrun run -- zsh -lc 'curl -s https://example.com | head -c 80'
stillrun run --shell 'npm run dev 2>&1 | tee dev.log'

# Search / inspect
stillrun history --query hello
stillrun history --status success --sort oldest
stillrun history --since 7d --branch main --exit-code 1 --json
stillrun inspect 1
stillrun inspect 1 --json

# Import shell history (preview first)
stillrun import-history --shell auto --preview
stillrun import-history --shell auto --yes

# Replay
stillrun replay 1 --preview
stillrun replay 1 --strict-context
stillrun replay 1 --yes

# Background Job
stillrun run --background --name demo-tick -- zsh -lc 'for i in 1 2 3; do echo tick-$i; sleep 1; done'
stillrun jobs
stillrun status demo-tick
stillrun logs demo-tick
stillrun logs demo-tick --max-bytes 10485760
stillrun jobs monitor demo-tick --once
stillrun jobs monitor demo-tick --background --name demo-monitor --interval-secs 5
stillrun stop demo-tick
stillrun jobs delete demo-tick

# Config & shell helpers
stillrun config show
stillrun hook install --shell auto
stillrun completion zsh > ~/.stillrun-completion.zsh
```

---

## Storage

Default paths:

```text
~/Library/Application Support/Stillrun/stillrun.db
~/Library/Application Support/Stillrun/logs/
~/Library/Application Support/Stillrun/config.toml
~/Library/LaunchAgents/com.stillrun.*.plist
```

| Variable | Purpose |
| --- | --- |
| `STILLRUN_HOME` | Override the Stillrun data directory |
| `STILLRUN_LAUNCH_AGENTS_DIR` | Override plist directory (tests / experiments) |

---

## Security

Stillrun redacts common secrets before writing to SQLite—env keys such as
`token` / `password` / `api_key`, and inline patterns like
`Authorization: Bearer ...`, `token=...`, `--token value`.

- **Replay** clears the current process environment, then restores recorded
  non-redacted values only. `--strict-context` fails fast when the recorded cwd
  is gone or the recorded Git branch/head no longer matches; Stillrun does not
  checkout Git state, restore TTY state, or recreate shell aliases/functions
  unless the command itself was captured as a shell command.
- **Imported** history requires `--preview` or `--yes` before replay.
- **Shell hooks** record command text, cwd, Git metadata, and exit code—not
  stdout/stderr. Use `stillrun run` or a Job when you need full output capture.
- **Background Jobs** refuse command-line secret values by default because
  launchd plists are files on disk. Move secrets into an external runtime source
  or secret manager and pass references instead.
- **Custom redaction keys** added with `stillrun config redact add KEY` are used
  for env capture, argv/command persistence, and stdout/stderr redaction. During
  foreground runs, sensitive env values from the current process are also scrubbed
  from captured output.

---

## Development

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

Release packaging and Homebrew tap maintenance live in [`packaging/`](packaging/README.md).

Real launchd lifecycle E2E (touches the user launchd session):

```bash
STILLRUN_RUN_LAUNCHD_E2E=1 cargo test --test launchd_e2e -- --nocapture
```

---

## License

MIT — see [`LICENSE`](LICENSE).
