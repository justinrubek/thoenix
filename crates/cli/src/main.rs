use clap::Parser;
use tracing::info;

mod commands;
mod error;
mod server;
mod terraform;

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
            let mut terraform = terraform.spawn_command().await?;
            let status = terraform.wait().await?;
            info!(?status);

            // If the process exited with a non-zero exit code, return an error
            if !status.success() {
                let code = status.code().expect("no exit code");
                return Err(error::AppError::TerraformError(code));
            }
        }
    }

    Ok(())
}
