use std::error::Error;

mod args;
mod commands;
mod webchat_server;

#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    commands::execute(args::parse_command()?).await
}
