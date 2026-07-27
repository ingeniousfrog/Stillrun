use clap::{Args, Subcommand, ValueEnum};

use crate::{db::Store, Result};

pub const STILLRUN_COMMANDS: &[&str] = &[
    "run",
    "history",
    "replay",
    "promote",
    "import-history",
    "jobs",
    "logs",
    "inspect",
    "status",
    "start",
    "stop",
    "restart",
    "hook",
    "config",
    "completion",
];

#[derive(Debug, Args)]
pub struct CompletionArgs {
    #[command(subcommand)]
    pub action: CompletionCommand,
}

#[derive(Debug, Subcommand)]
pub enum CompletionCommand {
    Bash,
    Zsh,
    Fish,
    Candidates(CompletionCandidateArgs),
}

#[derive(Debug, Args)]
pub struct CompletionCandidateArgs {
    #[arg(value_enum)]
    pub kind: CompletionCandidateKind,
    #[arg(long, default_value = "")]
    pub prefix: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CompletionCandidateKind {
    Commands,
    Jobs,
}

pub fn handle_completion_command(store: &Store, args: CompletionArgs) -> Result<()> {
    match args.action {
        CompletionCommand::Bash | CompletionCommand::Zsh | CompletionCommand::Fish => {
            print_completion_script(&args);
        }
        CompletionCommand::Candidates(candidate_args) => {
            let candidates = match candidate_args.kind {
                CompletionCandidateKind::Commands => command_candidates(&candidate_args.prefix),
                CompletionCandidateKind::Jobs => job_candidates(store, &candidate_args.prefix)?,
            };
            for candidate in candidates {
                println!("{candidate}");
            }
        }
    }
    Ok(())
}

pub fn print_completion_script(args: &CompletionArgs) -> bool {
    match args.action {
        CompletionCommand::Bash => {
            print!("{}", bash_script());
            true
        }
        CompletionCommand::Zsh => {
            print!("{}", zsh_script());
            true
        }
        CompletionCommand::Fish => {
            print!("{}", fish_script());
            true
        }
        CompletionCommand::Candidates(_) => false,
    }
}

pub fn command_candidates(prefix: &str) -> Vec<String> {
    STILLRUN_COMMANDS
        .iter()
        .copied()
        .filter(|command| prefix.is_empty() || command.starts_with(prefix))
        .map(String::from)
        .collect()
}

pub fn job_candidates(store: &Store, prefix: &str) -> Result<Vec<String>> {
    store.job_completion_candidates(prefix)
}

pub fn bash_script() -> &'static str {
    r#"# stillrun bash completion
_stillrun_complete() {
  local cur command
  COMPREPLY=()
  cur="${COMP_WORDS[COMP_CWORD]}"

  if [[ "$COMP_CWORD" -eq 1 ]]; then
    COMPREPLY=( $(compgen -W "run history replay promote import-history jobs logs inspect status start stop restart hook config completion" -- "$cur") )
    return 0
  fi

  command="${COMP_WORDS[1]}"
  case "$command" in
    logs|inspect|status|start|stop|restart)
      mapfile -t COMPREPLY < <(stillrun completion candidates jobs --prefix "$cur" 2>/dev/null)
      ;;
  esac
}
complete -F _stillrun_complete stillrun
"#
}

pub fn zsh_script() -> &'static str {
    r#"#compdef stillrun

_stillrun_jobs() {
  local -a jobs
  jobs=("${(@f)$(stillrun completion candidates jobs --prefix "$PREFIX" 2>/dev/null)}")
  _describe 'jobs' jobs
}

_stillrun() {
  local -a commands
  commands=(
    'run:run and record a foreground command'
    'history:search recorded command history'
    'replay:rerun a recorded execution'
    'promote:turn a recorded execution into a launchd job'
    'import-history:import local shell history'
    'jobs:list and manage background jobs'
    'logs:read job logs'
    'inspect:inspect an execution or job'
    'status:show job status'
    'start:start a job'
    'stop:stop a job'
    'restart:restart a job'
    'hook:install or print shell hooks'
    'config:manage Stillrun config'
    'completion:print shell completion scripts'
  )

  if (( CURRENT == 2 )); then
    _describe 'commands' commands
    return
  fi

  case "${words[2]}" in
    logs|inspect|status|start|stop|restart)
      _stillrun_jobs
      ;;
    *)
      _files
      ;;
  esac
}

_stillrun "$@"
"#
}

pub fn fish_script() -> &'static str {
    r#"# stillrun fish completion
complete -c stillrun -f -n '__fish_is_first_token' -a 'run history replay promote import-history jobs logs inspect status start stop restart hook config completion'
complete -c stillrun -f -n '__fish_seen_subcommand_from logs inspect status start stop restart' -a '(stillrun completion candidates jobs --prefix (commandline -ct) 2>/dev/null)'
"#
}
