#[derive(clap::Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Args {
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(clap::Subcommand, Debug)]
pub(crate) enum Commands {
    /// commands for running the git server
    Server(Server),
    /// commands for interacting with terraform.
    ///
    /// terraform will be invoked in the specified workspace's directory with the remaining arguments passed as-is,
    Terraform(Terraform),
}

#[derive(clap::Args, Debug)]
pub(crate) struct Server {
    #[clap(subcommand)]
    pub command: ServerCommands,

    /// the directory to store persistent data
    #[arg(long, short)]
    pub data_dir: std::path::PathBuf,
}

#[derive(clap::Subcommand, Debug)]
pub(crate) enum ServerCommands {
    /// start the server in http mode
    Http,
    /// start the server in ssh mode
    Ssh,
}

#[derive(clap::Args, Debug)]
pub(crate) struct Terraform {
    #[arg()]
    /// the name of the terraform configuration to use
    pub configuration_name: String,
    #[arg()]
    /// the arguments to pass to terraform
    pub args: Vec<String>,
    /// the command to run when invoking terraform
    #[arg(long, short, default_value = "tofu")]
    pub command: String,
}
