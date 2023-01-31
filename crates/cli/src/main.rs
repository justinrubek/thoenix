use russh::server::{Auth, Session};
use russh_keys::PublicKeyBase64;
use std::{collections::HashMap, net::SocketAddr, sync::Arc, path::PathBuf};
use tokio::{
    macros::support::Pin,
    io::{AsyncRead, AsyncWrite},
    sync::Mutex,
};

mod codec;
mod error;
use error::AppResult;
use tracing::info;

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
        SshSession::new(self.data_dir.clone())
    }
}

struct SshSession {
    clients: Arc<Mutex<HashMap<russh::ChannelId, russh::Channel<russh::server::Msg>>>>,
    data_dir: PathBuf,
}

impl SshSession {
    fn new(data_dir: PathBuf) -> Self {
        SshSession {
            clients: Arc::new(Mutex::new(HashMap::new())),
            data_dir,
        }
    }

    async fn get_channel(&mut self, channel_id: russh::ChannelId) -> russh::Channel<russh::server::Msg> {
        let mut clients = self.clients.lock().await;
        clients.remove(&channel_id).unwrap()
    }

    /// Respond with one line for each reference we currently have
    /// The first line also haas a list of the server's capabilities
    /// The data is transmitted in chunks.
    /// Each chunk starts with a 4 character hex value specifying the length of the chunk (including the 4 character hex value)
    /// Chunks usually contain a single line of data and a trailing linefeed
    #[tracing::instrument(skip(self, args))]
    async fn receive_pack(&mut self, channel_id: russh::ChannelId, args: Vec<&str>) -> AppResult<()> {
        info!(?args, ?self.data_dir, "git-receive-pack");
        // First, determine the repository name and path
        // We need to clean up the text from the url and make it a relative path to the data directory
        let repo_name = args[0].replace('\'', "").trim_start_matches('/').to_string();
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

        let channel = self.get_channel(channel_id).await;
        let _stream = channel.into_stream();

        // invoke git-receive-pack
        // send the output to the channel
        let _child = tokio::process::Command::new("git")
            .arg("receive-pack")
            .arg(&repo_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()?;

        // tokio::io::copy_bidirectional(&mut child_process, &mut stream).await?;

        // collect stdout
        // let mut stdout = child.stdout.unwrap();
        // let mut output = Vec::new();
        // tokio::io::copy(&mut stdout, &mut output).await?;
        // info!(?output);

        Ok(())
    }

    async fn upload_pack(&mut self, _channel_id: russh::ChannelId, args: Vec<&str>) -> AppResult<()> {
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
        session: Session,
    ) -> AppResult<(Self, Session)> {
        info!(?data, %channel_id, "exec request");
        let msg = russh::ChannelMsg::Exec {
            want_reply: true,
            command: data.into(),
        };
        let command_str = String::from_utf8_lossy(data);
        info!(?msg, %command_str, "sending exec request");

        // TODO: check command to be invoked
        fn parse_command(command: &str) -> Option<(&str, Vec<&str>)> {
            let mut parts = command.split_whitespace();
            let command = parts.next()?;
            let args = parts.collect::<Vec<_>>();

            Some((command, args))
        }

        match parse_command(&command_str) {
            Some(("git-receive-pack", args)) => {
                let _res = self.receive_pack(channel_id, args).await?;

                Ok((self, session))
            }
            Some(("git-upload-pack", args)) => {
                let _res = self.upload_pack(channel_id, args).await?;

                Ok((self, session))
            }
            Some((other, _args)) => {
                info!(%other, "unknown command");
                unimplemented!()
            }
            None => unimplemented!(),
        }
    }
}

#[tokio::main]
async fn main() -> AppResult<()> {
    tracing_subscriber::fmt::init();

    // first arg: the directory to store repositories in
    let data_dir = std::env::args().nth(1).ok_or(error::AppError::NoDataDir)?;
    // TODO: ensure data_dir exists

    let config = russh::server::Config {
        auth_rejection_time: std::time::Duration::from_secs(3),
        auth_rejection_time_initial: Some(std::time::Duration::from_secs(0)),
        keys: vec![russh_keys::key::KeyPair::generate_ed25519().unwrap()],
        connection_timeout: Some(std::time::Duration::from_secs(15)),
        ..Default::default()
    };

    let server = SshServer {
        data_dir: PathBuf::from(data_dir),
    };

    let address = (
        "127.0.0.10",
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
