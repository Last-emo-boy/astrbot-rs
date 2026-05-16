use std::error::Error;

use crate::args::CliCommand;

mod init;
mod run;
mod smoke;

pub(crate) async fn execute(command: CliCommand) -> Result<(), Box<dyn Error>> {
    match command {
        CliCommand::Smoke { config_path } => smoke::smoke(config_path).await,
        CliCommand::Init { config_path } => init::init(config_path).await,
        CliCommand::Run { config_path } => run::run(config_path).await,
    }
}
