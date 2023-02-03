use crate::{
    codec::TextChunkCodec,
    error::{self, AppResult},
};
use futures_util::sink::SinkExt;
use russh::server::{Auth, Session};
use std::{collections::HashMap, net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    macros::support::Pin,
    sync::Mutex,
};
use tokio_util::codec::{Decoder, FramedWrite};
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
pub(crate) struct SshServer {
    pub data_dir: PathBuf,
}

impl russh::server::Server for SshServer {
    type Handler = SshSession;

    fn new_client(&mut self, addr: Option<SocketAddr>) -> Self::Handler {
        info!(?addr, "new client");
        SshSession {
            clients: Arc::new(Mutex::new(HashMap::new())),
            data_dir: self.data_dir.clone(),

            child: None,
            child_stdin: None,

            input_buf: bytes::BytesMut::new(),
            codec: TextChunkCodec,
        }
    }
}

pub(crate) struct SshSession {
    clients: Arc<Mutex<HashMap<russh::ChannelId, russh::Channel<russh::server::Msg>>>>,
    data_dir: PathBuf,

    child: Option<tokio::task::JoinHandle<AppResult<()>>>,
    child_stdin: Option<tokio::process::ChildStdin>,

    input_buf: bytes::BytesMut,
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

    async fn cat(&mut self, channel_id: russh::ChannelId) -> AppResult<()> {
        let child = tokio::process::Command::new("cat")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()?;

        let channel = self.get_channel(channel_id).await;

        self.child_stdin = child.stdin;
        let mut child_stdout = child.stdout.unwrap();

        let task = tokio::spawn(async move {
            // tokio::io::copy(&mut child_stdout, &mut channel.into_stream()).await?;
            let mut stream = channel.into_stream();
            loop {
                let mut buf = [0u8; 1024];
                let n = child_stdout.read(&mut buf).await?;
                if n == 0 {
                    break;
                }
                info!(?n, "read");
                stream.write_all(&buf[..n]).await?;
            }
            Ok::<_, error::AppError>(())
        });
        self.child = Some(task);

        Ok(())
    }

    async fn receive_pack(
        &mut self,
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

        // invoke git-receive-pack
        // send the output to the channel
        let child = tokio::process::Command::new("git")
            .arg("receive-pack")
            .arg(&repo_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()?;

        let channel = self.get_channel(channel_id).await;

        self.child_stdin = child.stdin;
        let mut child_stdout = child.stdout.unwrap();

        let task = tokio::spawn(async move {
            tokio::io::copy(&mut child_stdout, &mut channel.into_stream()).await?;
            Ok::<_, error::AppError>(())
        });
        self.child = Some(task);

        Ok(())
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

    /// Our entrypoint for connections will be the `exec` command
    /// We will determine if the command is one we support and then'
    /// create a new task to handle the command
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
            Some(("git-receive-pack", args)) => self.receive_pack(channel_id, args).await,
            Some(("cat", _)) => self.cat(channel_id).await,
            Some((other, _args)) => {
                tracing::warn!(%other, "unknown command");
                session.channel_failure(channel_id);
                Err(error::AppError::UnsupportedCommand)
            }
            None => {
                tracing::warn!("no command");
                session.channel_failure(channel_id);
                Err(error::AppError::UnsupportedCommand)
            }
        }?;

        session.channel_success(channel_id);
        Ok((self, session))
    }

    /// Called with data is received from the client
    /// In order for data to be received, the channel must be established as successful
    async fn data(
        mut self,
        channel_id: russh::ChannelId,
        data: &[u8],
        mut session: russh::server::Session,
    ) -> AppResult<(Self, russh::server::Session)> {
        tracing::info!(%channel_id, "data");
        self.input_buf.extend_from_slice(data);

        let child_stdin = self
            .child_stdin
            .as_mut()
            .ok_or_else(|| error::AppError::MissingChild)?;

        // child_stdin.write_all(&data).await?;
        let mut child_stdin = FramedWrite::new(child_stdin, self.codec.clone());

        while let Some(frame) = self.codec.decode(&mut self.input_buf)? {
            if frame.is_empty() {
                // session.exit_status_request(channel_id, 0);
                // session.eof(channel_id);
                // session.close(channel_id);
                info!("client closed connection");
                return Ok((self, session));
            }

            child_stdin.send(frame).await?;
        }
        /*
         */

        Ok((self, session))
    }

    /*
    async fn channel_eof(
        mut self,
        channel_id: russh::ChannelId,
        session: russh::server::Session,
    ) -> AppResult<(Self, russh::server::Session)> {
        info!(%channel_id, "channel eof");
        let child = self.child.take().ok_or_else(|| error::AppError::MissingChild)?;

        child.abort();

        Ok((self, session))
    }
    */
}
