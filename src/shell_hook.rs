use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    context::CommandContext,
    db::{ExecutionStatus, NewExecution, Store},
    execution::now_ms,
    history_import::{ImportShellSelection, ShellKind},
    Result, StillrunError,
};

const BEGIN_MARKER: &str = "# >>> stillrun shell hook >>>";
const END_MARKER: &str = "# <<< stillrun shell hook <<<";

#[derive(Debug, Clone)]
pub struct ShellHookRecord {
    pub shell: ShellKind,
    pub command: String,
    pub cwd: PathBuf,
    pub started_at_ms: Option<i64>,
    pub exit_code: i32,
    pub source_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellHookInstallResult {
    pub shell: ShellKind,
    pub path: PathBuf,
}

pub fn record_shell_hook_execution(store: &Store, record: ShellHookRecord) -> Result<Option<i64>> {
    let command = record.command.trim();
    if command.is_empty() || should_ignore_hook_command(command) {
        return Ok(None);
    }

    let ended_at_ms = now_ms();
    let started_at_ms = record.started_at_ms.unwrap_or(ended_at_ms);
    let duration_ms = ended_at_ms.checked_sub(started_at_ms);
    let context = CommandContext::capture(&record.cwd);
    let source = format!("shell-hook:{}", record.shell.as_str());
    let source_id = record
        .source_id
        .unwrap_or_else(|| hook_source_id(&source, command, &record.cwd, started_at_ms));

    store.insert_sourced_execution(
        &NewExecution {
            argv: record.shell.replay_argv(command),
            context,
            started_at_ms,
            ended_at_ms: Some(ended_at_ms),
            duration_ms,
            exit_code: Some(record.exit_code),
            status: if record.exit_code == 0 {
                ExecutionStatus::Success
            } else {
                ExecutionStatus::Failed
            },
            stdout: String::new(),
            stderr: String::new(),
            pid: None,
            background_job_id: None,
            restart_count: 0,
        },
        &source,
        &source_id,
        command,
    )
}

pub fn install_shell_hook(selection: ImportShellSelection) -> Result<ShellHookInstallResult> {
    let shell = resolve_install_shell(selection)?;
    let home = user_home()?;
    let path = shell_rc_path(shell, &home);
    install_shell_hook_to_path(&path, shell)?;
    Ok(ShellHookInstallResult { shell, path })
}

pub fn install_shell_hook_to_path(path: &Path, shell: ShellKind) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let existing = fs::read_to_string(path).unwrap_or_default();
    let cleaned = remove_existing_hook_block(&existing);
    let block = wrapped_hook_script(shell);
    let updated = if cleaned.trim().is_empty() {
        format!("{block}\n")
    } else {
        format!("{}\n\n{block}\n", cleaned.trim_end())
    };
    fs::write(path, updated)?;
    Ok(())
}

pub fn shell_hook_script(shell: ShellKind) -> String {
    match shell {
        ShellKind::Zsh => zsh_hook_script(),
        ShellKind::Bash => bash_hook_script(),
        ShellKind::Fish => fish_hook_script(),
    }
}

pub fn shell_rc_path(shell: ShellKind, home: &Path) -> PathBuf {
    match shell {
        ShellKind::Zsh => std::env::var_os("ZDOTDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.to_path_buf())
            .join(".zshrc"),
        ShellKind::Bash => {
            let bashrc = home.join(".bashrc");
            if bashrc.exists() {
                bashrc
            } else {
                home.join(".bash_profile")
            }
        }
        ShellKind::Fish => std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"))
            .join("fish/config.fish"),
    }
}

fn wrapped_hook_script(shell: ShellKind) -> String {
    format!(
        "{BEGIN_MARKER}\n{}\n{END_MARKER}",
        shell_hook_script(shell).trim()
    )
}

fn remove_existing_hook_block(input: &str) -> String {
    let mut output = String::new();
    let mut in_block = false;
    for line in input.lines() {
        if line.trim() == BEGIN_MARKER {
            in_block = true;
            continue;
        }
        if line.trim() == END_MARKER {
            in_block = false;
            continue;
        }
        if !in_block {
            output.push_str(line);
            output.push('\n');
        }
    }
    output
}

fn resolve_install_shell(selection: ImportShellSelection) -> Result<ShellKind> {
    match selection {
        ImportShellSelection::Zsh => Ok(ShellKind::Zsh),
        ImportShellSelection::Bash => Ok(ShellKind::Bash),
        ImportShellSelection::Fish => Ok(ShellKind::Fish),
        ImportShellSelection::Auto => {
            let shell = std::env::var("SHELL").unwrap_or_default();
            if shell.ends_with("fish") {
                Ok(ShellKind::Fish)
            } else if shell.ends_with("bash") {
                Ok(ShellKind::Bash)
            } else {
                Ok(ShellKind::Zsh)
            }
        }
    }
}

fn user_home() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| StillrunError::invalid("HOME is not set"))
}

fn should_ignore_hook_command(command: &str) -> bool {
    let trimmed = command.trim_start();
    trimmed.starts_with("stillrun hook record")
}

fn hook_source_id(source: &str, command: &str, cwd: &Path, started_at_ms: i64) -> String {
    let input = format!("{source}\n{}\n{started_at_ms}\n{command}", cwd.display());
    format!("{started_at_ms}-{:016x}", stable_hash64(&input))
}

fn stable_hash64(input: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn zsh_hook_script() -> String {
    r#"
if command -v stillrun >/dev/null 2>&1; then
  autoload -Uz add-zsh-hook

  __stillrun_preexec() {
    [[ -n "${STILLRUN_INTERNAL:-}" ]] && return
    __STILLRUN_LAST_CMD="$1"
    __STILLRUN_LAST_CWD="$PWD"
    __STILLRUN_STARTED_MS="$(date +%s000)"
  }

  __stillrun_precmd() {
    local __stillrun_exit_code=$?
    [[ -z "${__STILLRUN_LAST_CMD:-}" ]] && return $__stillrun_exit_code
    STILLRUN_INTERNAL=1 command stillrun hook record --shell zsh --exit-code "$__stillrun_exit_code" --cwd "$__STILLRUN_LAST_CWD" --started-ms "$__STILLRUN_STARTED_MS" -- "$__STILLRUN_LAST_CMD" >/dev/null 2>&1
    unset __STILLRUN_LAST_CMD __STILLRUN_LAST_CWD __STILLRUN_STARTED_MS
    return $__stillrun_exit_code
  }

  add-zsh-hook preexec __stillrun_preexec
  add-zsh-hook precmd __stillrun_precmd
fi
"#
    .trim()
    .to_string()
}

fn bash_hook_script() -> String {
    r#"
if command -v stillrun >/dev/null 2>&1; then
  __stillrun_debug_trap() {
    local __stillrun_cmd="${BASH_COMMAND:-}"
    [[ -n "${STILLRUN_INTERNAL:-}" ]] && return
    [[ "$__stillrun_cmd" == __stillrun_* ]] && return
    [[ "$__stillrun_cmd" == "stillrun hook record"* ]] && return
    __STILLRUN_LAST_CMD="$__stillrun_cmd"
    __STILLRUN_LAST_CWD="$PWD"
    __STILLRUN_STARTED_MS="$(date +%s000)"
  }

  __stillrun_prompt_command() {
    local __stillrun_exit_code=$?
    if [[ -n "${__STILLRUN_LAST_CMD:-}" ]]; then
      STILLRUN_INTERNAL=1 command stillrun hook record --shell bash --exit-code "$__stillrun_exit_code" --cwd "$__STILLRUN_LAST_CWD" --started-ms "$__STILLRUN_STARTED_MS" -- "$__STILLRUN_LAST_CMD" >/dev/null 2>&1
      unset __STILLRUN_LAST_CMD __STILLRUN_LAST_CWD __STILLRUN_STARTED_MS
    fi
    return $__stillrun_exit_code
  }

  trap '__stillrun_debug_trap' DEBUG
  case ";${PROMPT_COMMAND:-};" in
    *";__stillrun_prompt_command;"*) ;;
    *) PROMPT_COMMAND="__stillrun_prompt_command${PROMPT_COMMAND:+;$PROMPT_COMMAND}" ;;
  esac
fi
"#
    .trim()
    .to_string()
}

fn fish_hook_script() -> String {
    r#"
if type -q stillrun
  function __stillrun_preexec --on-event fish_preexec
    set -g __STILLRUN_LAST_CMD "$argv[1]"
    set -g __STILLRUN_LAST_CWD "$PWD"
    set -g __STILLRUN_STARTED_MS (date +%s000)
  end

  function __stillrun_postexec --on-event fish_postexec
    set -l __stillrun_exit_code $status
    if test -n "$__STILLRUN_LAST_CMD"
      env STILLRUN_INTERNAL=1 stillrun hook record --shell fish --exit-code "$__stillrun_exit_code" --cwd "$__STILLRUN_LAST_CWD" --started-ms "$__STILLRUN_STARTED_MS" -- "$__STILLRUN_LAST_CMD" >/dev/null 2>&1
      set -e __STILLRUN_LAST_CMD
      set -e __STILLRUN_LAST_CWD
      set -e __STILLRUN_STARTED_MS
    end
    return $__stillrun_exit_code
  end
end
"#
    .trim()
    .to_string()
}
