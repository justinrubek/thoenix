use russh::server::{Auth, Session};
use russh_keys::PublicKeyBase64;
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;

mod error;
use error::AppResult;
use tracing::info;

#[derive(Clone, Debug)]
struct SshServer;

impl russh::server::Server for SshServer {
    type Handler = SshSession;

    fn new_client(&mut self, addr: Option<SocketAddr>) -> Self::Handler {
        info!(?addr, "new client");
        SshSession::default()
    }
}

struct SshSession {
    clients: Arc<Mutex<HashMap<russh::ChannelId, russh::Channel<russh::server::Msg>>>>,
}

impl Default for SshSession {
    fn default() -> Self {
        SshSession {
            clients: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

/// Respond with one line for each reference we currently have
/// The first line also haas a list of the server's capabilities
/// The data is transmitted in chunks.
/// Each chunk starts with a 4 character hex value specifying the length of the chunk (including the 4 character hex value)
/// Chunks usually contain a single line of data and a trailing linefeed
async fn receive_pack(args: Vec<&str>) -> AppResult<()> {
    info!(?args, "git-receive-pack");
    // TODO: First, determine the repository name and path

    // TODO: Is it enough to just invoke the command from the proper directory?
    Ok(())
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
                let _res = receive_pack(args).await?;

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

    let config = russh::server::Config {
        auth_rejection_time: std::time::Duration::from_secs(3),
        auth_rejection_time_initial: Some(std::time::Duration::from_secs(0)),
        keys: vec![russh_keys::key::KeyPair::generate_ed25519().unwrap()],
        connection_timeout: Some(std::time::Duration::from_secs(15)),
        ..Default::default()
    };

    let server = SshServer;

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
