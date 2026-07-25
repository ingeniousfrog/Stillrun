# Stillrun

[中文文档](README.zh-CN.md)

Stillrun is a command lifecycle runtime for macOS. It is not another shell
history tool and it is not another PM2 clone: the core idea is that every
terminal command can become a Job with history, logs, status, replay, and a
manageable lifecycle.

The MVP focuses on a native CLI surface and launchd-backed persistence. There is
no TUI, no Web UI, and no AI layer in the first phase.

## What Works Now

- `stillrun run -- <command...>` executes a foreground command, captures stdout,
  stderr, exit status, duration, PID, cwd, Git repository, Git branch, and a
  redacted environment snapshot.
- `stillrun history` lists recorded executions.
- `stillrun history --query <text>` searches commands and captured output with
  SQLite FTS5.
- `stillrun history --since-ms <epoch-ms> --until-ms <epoch-ms>` narrows history
  by execution start time.
- `stillrun import-history --shell auto` imports existing local zsh, bash, and
  fish shell history into Stillrun after explicit user consent.
- `stillrun replay <id>` reruns a prior command from its original cwd with the
  captured non-sensitive environment, avoiding accidental leakage from the
  current shell environment.
- `stillrun promote <id> --name <name>` turns a previous execution into a
  launchd-backed background Job while preserving cwd and non-sensitive
  environment variables.
- `stillrun inspect <id>` shows execution metadata.
- `stillrun run --background --name <name> -- <command...>` writes a launchd
  plist, stores a Job record, and bootstraps it through `launchctl` on macOS.
  One-shot background commands do not auto-restart by default.
- `stillrun run --background --keep-alive --name <name> -- <command...>` asks
  launchd to keep a service-style Job alive after exit.
- `stillrun jobs` lists known Jobs and tries to enrich them with live launchd
  status, PID, CPU percent, RSS memory, and restart count.
- `stillrun status <job>` shows one Job's stored metadata plus synchronized
  runtime status and log paths.
- `stillrun logs <job>` reads the launchd stdout/stderr log files. Follow mode
  can wait on log files that have not been created by launchd yet.
- `stillrun start <job>`, `stillrun stop <job>`, and `stillrun restart <job>`
  map to launchd bootout/bootstrap operations.

## Install From Source

```sh
./scripts/install.sh
```

The install script uses `cargo install --path . --force`, then asks whether to
import existing local shell history. To skip the prompt, install directly:

```sh
cargo install --path .
```

For local development:

```sh
cargo test
cargo run -- run -- printf "hello stillrun\n"
cargo run -- history --query stillrun
```

## CLI Examples

Run and record a command:

```sh
stillrun run -- npm run dev
```

Search by command text or captured output:

```sh
stillrun history --query "npm"
stillrun history --cwd /path/to/project
stillrun history --status success
stillrun history --since-ms 1720000000000 --until-ms 1730000000000
```

Import existing shell history after install:

```sh
stillrun import-history --shell auto
stillrun import-history --shell zsh --file ~/.zsh_history
stillrun history --status imported --query "npm"
```

Replay an execution:

```sh
stillrun replay 1
```

Promote a historical command into a background Job:

```sh
stillrun run -- python scripts/download.py
stillrun history --query download.py
stillrun promote 1 --name downloader
stillrun logs downloader --follow
```

Create a persistent background Job:

```sh
stillrun run --background --name dev-server -- npm run dev
stillrun run --background --keep-alive --name api-server -- cargo run
stillrun jobs
stillrun status dev-server
stillrun logs dev-server --follow
stillrun stop dev-server
stillrun start dev-server
stillrun restart dev-server
```

For shell features such as pipes, redirects, globbing, or compound commands,
call the shell explicitly:

```sh
stillrun run -- zsh -lc "curl -s https://example.com | jq ."
```

## Storage

By default, Stillrun stores data at:

```text
~/Library/Application Support/Stillrun/stillrun.db
~/Library/Application Support/Stillrun/logs/
~/Library/Application Support/Stillrun/config.toml
```

Set `STILLRUN_HOME` to isolate state during testing or development:

```sh
STILLRUN_HOME=/tmp/stillrun-dev cargo run -- history
```

Background Jobs are represented as launchd plists under
`~/Library/LaunchAgents` by default. Set `STILLRUN_LAUNCH_AGENTS_DIR` when you
need an alternate plist directory for tests or experiments. macOS launchd may
reject plists loaded from insecure temporary directories, so end-to-end
background lifecycle checks should use the real LaunchAgents location.

## Architecture

- CLI: `clap`
- Runtime: `tokio`
- Storage: SQLite with FTS5 through `rusqlite`
- Config: `serde` + TOML
- Logs: `tracing`
- File watching extension point: `notify`
- Process execution: `tokio::process`
- Background persistence on macOS: `launchd`, via generated plist files and
  `launchctl`

The code is split by lifecycle responsibility:

- `src/execution.rs`: foreground execution and replay
- `src/db.rs`: SQLite schema, history, jobs, and FTS search
- `src/context.rs`: cwd, Git, and environment capture
- `src/redact.rs`: secret redaction before persistence
- `src/jobs/`: launchd plist generation, bootstrap/bootout, runtime status, and
  resource sampling. Resource sampling is best-effort: if `ps` is blocked by the
  caller's environment, Stillrun still preserves launchd status, PID, exit code,
  and restart count.
- `src/cli.rs`: user-facing command surface

## Security Boundary

Stillrun redacts common sensitive environment keys and inline command/output
patterns before writing to SQLite. Examples include tokens, passwords, secrets,
API keys, `Authorization: Bearer ...`, and `token=...` style assignments.
Structured argv values are redacted before persistence too, so flags such as
`--token secret` and `--password=hunter2` are stored as redacted arguments in
history and Job records.

Replay clears the current process environment before restoring the captured
non-redacted environment values. Redacted secrets are not reintroduced by
design, and redacted argv values are replayed as `[redacted]` rather than the
original secret.

Promoted Jobs use the same policy: launchd receives the original cwd and
non-sensitive environment values, while redacted keys and values are omitted
from the generated plist.

## MVP Boundaries

Stillrun currently prioritizes macOS. Background Job persistence intentionally
uses launchd rather than a custom daemon. Linux and Windows support can reuse
the same storage and CLI model later, but non-macOS background lifecycle
operations return an explicit unsupported error in the MVP.

By default, `--background` creates a persistent launchd-managed Job without
automatic restart. Use `--keep-alive` for long-running services that should be
restarted by launchd after exit.

Runtime restart counts are synchronized from launchd `runs` where available and
merged with manual `stillrun restart` counts without lowering an existing value.

`stillrun stop` is idempotent for already-unloaded launchd services: Stillrun
marks the Job stopped if launchd reports that the service no longer exists, but
still surfaces unrelated launchd failures.

`stillrun start` checks launchd before acting: missing services are bootstrapped
from the stored plist, loaded-but-stopped services are started with
`launchctl kickstart`, and already-running services are left running without a
forced restart.
