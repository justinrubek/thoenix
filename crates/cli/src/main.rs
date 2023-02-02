use russh_keys::PublicKeyBase64;
use std::{path::PathBuf, sync::Arc};
use tokio::io::AsyncWriteExt;
use tracing::info;

mod error;
mod handler;

use error::AppResult;
use handler::SshServer;

/// Load a keypair from a file, or generate a new one if it doesn't exist.
async fn get_keypair(data_dir: &str) -> AppResult<russh_keys::key::KeyPair> {
    let key_path = std::path::Path::new(&data_dir).join("id_rsa");
    if !key_path.exists() {
        // generate a keypair if none exists
        let keys = russh_keys::key::KeyPair::generate_ed25519().unwrap();
        let mut key_file = tokio::fs::File::create(&key_path).await?;

        let russh_keys::key::KeyPair::Ed25519(inner_pair) = keys;

        key_file.write_all(&inner_pair.to_bytes()).await?;

        Ok(russh_keys::key::KeyPair::Ed25519(inner_pair))
    } else {
        // load the keypair from the file
        let key_data = tokio::fs::read(&key_path).await?;
        let keypair = ed25519_dalek::Keypair::from_bytes(&key_data)?;

        Ok(russh_keys::key::KeyPair::Ed25519(keypair))
    }
}

#[tokio::main]
async fn main() -> AppResult<()> {
    tracing_subscriber::fmt::init();

    // first arg: the directory to store repositories in
    let data_dir = std::env::args().nth(1).ok_or(error::AppError::NoDataDir)?;
    // TODO: ensure data_dir exists

    let keys = get_keypair(&data_dir).await?;

    let config = russh::server::Config {
        auth_rejection_time: std::time::Duration::from_secs(3),
        auth_rejection_time_initial: Some(std::time::Duration::from_secs(0)),
        keys: vec![keys],
        connection_timeout: Some(std::time::Duration::from_secs(15)),
        ..Default::default()
    };

    let server = SshServer {
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
