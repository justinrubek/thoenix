use tokio::io::AsyncWriteExt;

use crate::error::Result;

/// Load a keypair from a file, or generate a new one if it doesn't exist.
pub async fn get_or_generate_keypair(data_dir: &str) -> Result<russh_keys::key::KeyPair> {
    let key_path = std::path::Path::new(&data_dir).join("id_rsa");
    if !key_path.exists() {
        // generate a keypair if none exists
        let keys = russh_keys::key::KeyPair::generate_ed25519();
        let mut key_file = tokio::fs::File::create(&key_path).await?;

        let russh_keys::key::KeyPair::Ed25519(inner_pair) = keys else {
            panic!("failure generating key")
        };

        key_file.write_all(&inner_pair.to_bytes()).await?;

        Ok(russh_keys::key::KeyPair::Ed25519(inner_pair))
    } else {
        // load the keypair from the file
        let key_data = tokio::fs::read(&key_path).await?;
        let key_bytes: [u8; 32] = key_data.try_into().unwrap();
        let keypair = ed25519_dalek::SigningKey::from_bytes(&key_bytes);

        Ok(russh_keys::key::KeyPair::Ed25519(keypair))
    }
}
