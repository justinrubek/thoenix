use clap::Parser;
use std::error::Error;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod commands;
mod error;
mod server;
mod terraform;

use commands::{Commands, ServerCommands};
use error::AppResult;
use server::Server;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "thoenix_http=debug,tower_http=debug,axum::rejection=trace".into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

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

            if !status.success() {
                let code = status.code().expect("no exit code");
                return Err(error::AppError::TerraformError(code).into());
            }
        }
    }

    Ok(())
}
