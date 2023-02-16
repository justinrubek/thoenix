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
            info!(?workspace, ?args, "spawning terraform command");

            // Call the terraform executable with the provided args
            // The workspace will determine the directory to run terraform in (using the -chdir flag)
            // The args will be passed to terraform as-is
            let mut terraform = tokio::process::Command::new("terraform")
                .arg(format!("-chdir={workspace}"))
                .args(args)
                .spawn()?;

            // From here, let the process take over. Display the output from it, both stdout and stderr
            let status = terraform.wait().await?;
            info!(?status);

            // If the process exited with a non-zero exit code, return an error
            if !status.success() {
                let code = status.code();
                return Err(error::AppError::TerraformError(code.expect("no exit code")));
            }
        }
    }

    Ok(())
}
