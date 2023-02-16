use crate::error::AppResult;
use russh_keys::PublicKeyBase64;
use std::{path::PathBuf, sync::Arc};
use tracing::info;

pub(crate) struct Server {
    data_dir: PathBuf,
}

impl Server {
    pub(crate) fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    /// experimental ssh server, functionality is not complete
    #[allow(dead_code)]
    pub(crate) async fn ssh_server(self) -> AppResult<()> {
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

    pub(crate) async fn http_server(self) -> AppResult<()> {
        let server = thoenix_http::Server::new(self.data_dir);

        let port = std::env::var("PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse()
            .unwrap();

        server.run(port).await?;

        Ok(())
    }
}
