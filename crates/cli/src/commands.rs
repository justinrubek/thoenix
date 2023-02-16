#[derive(clap::Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Args {
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(clap::Subcommand, Debug)]
pub(crate) enum Commands {
    /// commands for running the scheduling server
    Server(Server),
    /// commands for interacting with terraform
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
    // capture all args passed in to a single string
    #[arg()]
    pub args: Vec<String>,
}
