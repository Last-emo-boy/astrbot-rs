use std::error::Error;
use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CliCommand {
    Smoke { config_path: Option<PathBuf> },
    Init { config_path: PathBuf },
    Run { config_path: PathBuf },
}

pub(crate) fn parse_command() -> Result<CliCommand, Box<dyn Error>> {
    parse_command_from(std::env::args().skip(1))
}

pub(crate) fn parse_command_from(
    args: impl IntoIterator<Item = String>,
) -> Result<CliCommand, Box<dyn Error>> {
    let mut args = args.into_iter();
    let command = args.next();

    Ok(match command.as_deref() {
        None => CliCommand::Smoke { config_path: None },
        Some("smoke") => CliCommand::Smoke {
            config_path: args.next().map(PathBuf::from),
        },
        Some("init") => CliCommand::Init {
            config_path: args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(default_config_path),
        },
        Some("run") => CliCommand::Run {
            config_path: args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(default_config_path),
        },
        Some(other) => {
            return Err(
                format!("unknown command {other}. Expected one of: smoke, init, run").into(),
            );
        }
    })
}

pub(crate) fn default_config_path() -> PathBuf {
    PathBuf::from("astrbot.runtime.json")
}
