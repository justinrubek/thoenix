use futures::{SinkExt, TryStreamExt};
use russh::server::{Auth, Session};
use russh_keys::PublicKeyBase64;
use std::{collections::HashMap, net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    macros::support::Pin,
    sync::Mutex,
};
use tokio_util::codec::{Decoder, Encoder, Framed};
use tracing::info;

mod codec;
mod error;
use codec::TextChunkCodec;
use error::AppResult;

/// A thin wrapper around tokio::process::Child that implements AsyncRead
/// and AsyncWrite on top of the child's stdout and stdin.
struct ChildProcess {
    inner: tokio::process::Child,
}

impl AsyncRead for ChildProcess {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner.stdout.as_mut().unwrap()).poll_read(cx, buf)
    }
}

impl AsyncWrite for ChildProcess {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        info!(?buf, "poll_write");
        Pin::new(&mut self.inner.stdin.as_mut().unwrap()).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        info!("poll_flush");
        Pin::new(&mut self.inner.stdin.as_mut().unwrap()).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        info!("poll_shutdown");
        Pin::new(&mut self.inner.stdin.as_mut().unwrap()).poll_shutdown(cx)
    }
}

#[derive(Clone, Debug)]
struct SshServer {
    data_dir: PathBuf,
}

impl russh::server::Server for SshServer {
    type Handler = SshSession;

    fn new_client(&mut self, addr: Option<SocketAddr>) -> Self::Handler {
        info!(?addr, "new client");
        SshSession {
            clients: Arc::new(Mutex::new(HashMap::new())),
            data_dir: self.data_dir.clone(),

            input_buf: bytes::BytesMut::new(),
            output_buf: bytes::BytesMut::new(),

            codec: TextChunkCodec,
        }
    }
}

struct SshSession {
    clients: Arc<Mutex<HashMap<russh::ChannelId, russh::Channel<russh::server::Msg>>>>,
    data_dir: PathBuf,

    input_buf: bytes::BytesMut,
    output_buf: bytes::BytesMut,

    codec: TextChunkCodec,
}

impl SshSession {
    async fn get_channel(
        &mut self,
        channel_id: russh::ChannelId,
    ) -> russh::Channel<russh::server::Msg> {
        let mut clients = self.clients.lock().await;
        clients.remove(&channel_id).unwrap()
    }

    async fn write(&mut self, data: String) -> AppResult<()> {
        self.codec.encode(data, &mut self.output_buf)
    }

    async fn flush(
        &mut self,
        session: &mut russh::server::Session,
        channel_id: russh::ChannelId,
    ) -> AppResult<()> {
        session.data(
            channel_id,
            russh::CryptoVec::from_slice(&self.output_buf.split().as_ref()),
        );

        Ok(())
    }

    /// Respond with one line for each reference we currently have
    /// The first line also haas a list of the server's capabilities
    /// The data is transmitted in chunks.
    /// Each chunk starts with a 4 character hex value specifying the length of the chunk (including the 4 character hex value)
    /// Chunks usually contain a single line of data and a trailing linefeed
    #[tracing::instrument(skip(self, args, session))]
    async fn receive_pack(
        &mut self,
        session: &mut russh::server::Session,
        channel_id: russh::ChannelId,
        args: Vec<&str>,
    ) -> AppResult<()> {
        info!(?args, ?self.data_dir, "git-receive-pack");
        // First, determine the repository name and path
        // We need to clean up the text from the url and make it a relative path to the data directory
        let repo_name = args[0]
            .replace('\'', "")
            .trim_start_matches('/')
            .to_string();
        let repo_path = self.data_dir.join(repo_name);
        info!(?repo_path);

        // Next, we need to create the repository if it doesn't exist
        if !repo_path.exists() {
            // assume a `git` command is available to create the repository
            tokio::process::Command::new("git")
                .arg("init")
                .arg("--bare")
                .arg(&repo_path)
                .output()
                .await?;
        }

        let mut channel = self.get_channel(channel_id).await;

        // invoke git-receive-pack
        // send the output to the channel
        let child = tokio::process::Command::new("git")
            .arg("receive-pack")
            .arg(&repo_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()?;

        // tokio::io::copy_bidirectional(&mut child_process, &mut stream).await?;
        let child_process = ChildProcess { inner: child };

        let mut server = Framed::new(child_process, TextChunkCodec);
        // read from stream, any input

        loop {
            let chunk = server.try_next().await?;
            info!(?chunk);
            if let Some(chunk) = chunk {
                if chunk.is_empty() {
                    // Send the final empty chunk to indicate the end of the stream
                    self.write(chunk).await?;
                    break;
                }
                self.write(chunk).await?;
            }
        }

        info!("done reading from child process");
        self.flush(session, channel_id).await?;

        // Now, use the channel to receive data
        let response = channel.wait().await.unwrap();
        info!(?response);

        // collect stdout
        // let mut stdout = child.stdout.unwrap();
        // let mut output = Vec::new();
        // tokio::io::copy(&mut stdout, &mut output).await?;
        // info!(?output);

        Ok(())
    }

    async fn cat(
        &mut self,
        session: &mut russh::server::Session,
        channel_id: russh::ChannelId,
    ) -> AppResult<()> {
        info!("cat");

        let child = tokio::process::Command::new("cat")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()?;

        let channel = self.get_channel(channel_id).await;
        let mut stream = channel.into_stream();

        let mut buf = bytes::BytesMut::new();
        loop {
            info!("reading from channel");
            // let chunk = channel.wait().await.ok_or_else(|| anyhow::anyhow!("channel closed"))?;
            let chunk = stream.read(&mut buf).await?;
            info!(?chunk, "read from channel");
        }

        self.flush(session, channel_id).await?;

        // Now, use the channel to receive data

        Ok(())
    }

    async fn upload_pack(
        &mut self,
        _channel_id: russh::ChannelId,
        args: Vec<&str>,
    ) -> AppResult<()> {
        info!(?args, ?self.data_dir, "git-upload-pack");

        todo!()
    }
}

#[async_trait::async_trait]
impl russh::server::Handler for SshSession {
    type Error = error::AppError;

    async fn auth_password(self, user: &str, password: &str) -> AppResult<(Self, Auth)> {
        info!(?user, ?password, "auth password");
        Ok((self, Auth::Accept))
    }

    async fn auth_publickey(
        self,
        user: &str,
        public_key: &russh_keys::key::PublicKey,
    ) -> AppResult<(Self, Auth)> {
        info!(%user, ?public_key, "auth public key");
        Ok((self, Auth::Accept))
    }

    async fn channel_open_session(
        mut self,
        channel: russh::Channel<russh::server::Msg>,
        session: Session,
    ) -> AppResult<(Self, bool, Session)> {
        let channel_id = channel.id();
        info!(?channel_id, "channel open session");
        {
            let mut clients = self.clients.lock().await;
            clients.insert(channel.id(), channel);
        }
        Ok((self, true, session))
    }

    async fn subsystem_request(
        mut self,
        channel_id: russh::ChannelId,
        name: &str,
        mut session: Session,
    ) -> AppResult<(Self, Session)> {
        info!(%name, "subsystem request");

        session.channel_failure(channel_id);

        Ok((self, session))
    }

    async fn exec_request(
        mut self,
        channel_id: russh::ChannelId,
        data: &[u8],
        mut session: Session,
    ) -> AppResult<(Self, Session)> {
        info!(%channel_id, "exec request");
        let command_str = String::from_utf8_lossy(data);
        info!(%command_str, "sending exec request");

        fn parse_command(command: &str) -> Option<(&str, Vec<&str>)> {
            let mut parts = command.split_whitespace();
            let command = parts.next()?;
            let args = parts.collect::<Vec<_>>();

            Some((command, args))
        }

        match parse_command(&command_str) {
            Some(("git-receive-pack", args)) => {
                let _res = self.receive_pack(&mut session, channel_id, args).await?;
            }
            Some(("git-upload-pack", args)) => {
                let _res = self.upload_pack(channel_id, args).await?;
            }
            Some(("cat", _)) => {
                let _res = self.cat(&mut session, channel_id).await?;
                session.close(channel_id);
            }
            Some((other, _args)) => {
                info!(%other, "unknown command");
                unimplemented!()
            }
            None => unimplemented!(),
        }

        Ok((self, session))
    }
}

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
