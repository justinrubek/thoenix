use clap::Parser;
use tracing::info;

mod commands;
mod error;
mod server;

use commands::{Commands, ServerCommands};
use error::AppResult;
use server::Server;

#[tokio::main]
async fn main() -> AppResult<()> {
    tracing_subscriber::fmt::init();

    // process commands
    let args = commands::Args::parse();
    match args.command {
        Commands::Server(server) => {
            let cmd = server.command;
            let server = Server::new(server.data_dir);

            match cmd {
                ServerCommands::Http => server.http_server().await?,
                ServerCommands::Ssh => server.ssh_server().await?,
            }
        }
        Commands::Terraform(terraform) => {
            let (workspace, args) = terraform
                .args
                .split_first()
                .ok_or_else(|| error::AppError::InvalidArgs("no terraform args".to_string()))?;

            // read all args and print them
            info!(?workspace, ?args);

            // Call the terraform executable with the provided args
            // The workspace will determine the directory to run terraform in (using the -chdir flag)
            todo!()
        }
    }
    Ok(())
}
