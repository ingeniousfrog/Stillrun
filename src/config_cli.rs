use clap::{Args, Subcommand};

use crate::{config::StillrunConfig, paths::StillrunPaths, Result};

#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: ConfigCommand,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    Show(ConfigShowArgs),
    Path,
    Set(ConfigSetArgs),
    Redact(ConfigRedactArgs),
}

#[derive(Debug, Args)]
pub struct ConfigShowArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ConfigSetArgs {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Args)]
pub struct ConfigRedactArgs {
    #[command(subcommand)]
    pub action: ConfigRedactCommand,
}

#[derive(Debug, Subcommand)]
pub enum ConfigRedactCommand {
    List,
    Add(ConfigRedactKeyArgs),
    Remove(ConfigRedactKeyArgs),
}

#[derive(Debug, Args)]
pub struct ConfigRedactKeyArgs {
    pub key: String,
}

pub fn handle_config_command(
    paths: &StillrunPaths,
    mut config: StillrunConfig,
    args: ConfigArgs,
) -> Result<()> {
    match args.action {
        ConfigCommand::Show(show_args) => {
            if show_args.json {
                println!("{}", serde_json::to_string(&config)?);
            } else {
                print!("{}", toml::to_string_pretty(&config)?);
            }
        }
        ConfigCommand::Path => {
            println!("{}", paths.config_path.display());
        }
        ConfigCommand::Set(set_args) => {
            config.set_value(&set_args.key, &set_args.value)?;
            config.save(paths)?;
            println!("{}={}", set_args.key.replace('-', "_"), set_args.value);
        }
        ConfigCommand::Redact(redact_args) => match redact_args.action {
            ConfigRedactCommand::List => {
                for key in &config.redact_keys {
                    println!("{key}");
                }
            }
            ConfigRedactCommand::Add(add_args) => {
                let inserted = config.add_redact_key(&add_args.key)?;
                config.save(paths)?;
                if inserted {
                    println!(
                        "added redact key {}",
                        normalized_key_for_output(&add_args.key)
                    );
                } else {
                    println!(
                        "redact key {} already exists",
                        normalized_key_for_output(&add_args.key)
                    );
                }
            }
            ConfigRedactCommand::Remove(remove_args) => {
                let removed = config.remove_redact_key(&remove_args.key)?;
                config.save(paths)?;
                if removed {
                    println!(
                        "removed redact key {}",
                        normalized_key_for_output(&remove_args.key)
                    );
                } else {
                    println!(
                        "redact key {} was not configured",
                        normalized_key_for_output(&remove_args.key)
                    );
                }
            }
        },
    }
    Ok(())
}

fn normalized_key_for_output(key: &str) -> String {
    key.trim().to_ascii_lowercase()
}
