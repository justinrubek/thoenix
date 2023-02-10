use russh_keys::PublicKeyBase64;
use std::{path::PathBuf, sync::Arc};
use tracing::info;

mod error;

use error::AppResult;

struct ThoenixServer {
    data_dir: PathBuf,
}

impl ThoenixServer {
    fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    /// experimental ssh server, functionality is not complete
    async fn ssh_server(self) -> AppResult<()> {
        let data_dir = self.data_dir.to_str().unwrap();

        let keys = thoenix_ssh::util::get_or_generate_keypair(data_dir).await?;

        let config = russh::server::Config {
            auth_rejection_time: std::time::Duration::from_secs(3),
            auth_rejection_time_initial: Some(std::time::Duration::from_secs(0)),
            keys: vec![keys],
            connection_timeout: Some(std::time::Duration::from_secs(30)),
            ..Default::default()
        };

        let server = thoenix_ssh::handler::SshServer {
            data_dir: PathBuf::from(data_dir),
        };

        let address = (
            "127.0.0.0",
            std::env::var("PORT")
                .unwrap_or_else(|_| "2222".to_string())
                .parse()
                .unwrap(),
        );

        let public_key = config.keys[0].public_key_base64();
        info!(%public_key);

        info!(?address, "starting server");
        russh::server::run(Arc::new(config), address, server).await?;

        Ok(())
    }
}

#[tokio::main]
async fn main() -> AppResult<()> {
    tracing_subscriber::fmt::init();

    // first arg: the directory to store repositories in
    let data_dir = std::env::args().nth(1).ok_or(error::AppError::NoDataDir)?;
    // TODO: ensure data_dir exists

    let server = ThoenixServer::new(data_dir.into());
    server.ssh_server().await?;

    Ok(())
}
