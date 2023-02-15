use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    routing::{get, post},
    Router,
};
use std::{collections::HashMap, net::SocketAddr, path::PathBuf, sync::Arc};
use tokio_util::codec::Encoder;
use tracing::info;

pub mod codec;
pub mod error;
pub mod message;

use error::Result;
use message::GitCodec;

pub struct ServerState {
    pub repo_path: PathBuf,
}

pub struct Server {
    #[allow(dead_code)]
    data_dir: PathBuf,
}

impl Server {
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    pub async fn run(&self, port: u16) -> Result<()> {
        let cors = tower_http::cors::CorsLayer::permissive();

        let addr = SocketAddr::from(([0, 0, 0, 0], port));

        let app_state = Arc::new(ServerState {
            repo_path: self.data_dir.clone(),
        });

        let app = Router::new()
            .route("/", get(root))
            // .route("/configs/:owner/:repo.git/info/refs", get(list_refs_child))
            .route("/configs/:owner/:repo.git/info/refs", get(list_refs))
            .route(
                "/configs/:owner/:repo.git/git-receive-pack",
                post(receive_pack),
            )
            .with_state(app_state)
            .layer(cors);

        println!("Listening on {addr}");
        axum::Server::bind(&addr)
            .serve(app.into_make_service())
            .await?;

        Ok(())
    }
}

async fn root() -> &'static str {
    "Hello, World!"
}

// Generate a response for git-receive-pack using the `git` executable
async fn _list_refs_child(
    State(app_state): State<Arc<ServerState>>,
    Path((owner, repo)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
    payload: Bytes,
) -> Result<impl axum::response::IntoResponse> {
    info!("Received data from {}/{}", owner, repo);
    info!("Payload: {:?}", payload);

    // We need to respond with the results for `git receive-pack` to work

    let service = query.get("service").ok_or(error::Error::MissingService)?;
    info!("Service: {}", service);

    // determine the path to the repo
    let repo_path = app_state.repo_path.join(owner).join(repo);

    // Call `git receive-pack --http-backend-info-refs` to get the data
    let child = tokio::process::Command::new("git")
        .arg("receive-pack")
        .arg("--http-backend-info-refs")
        .arg(&repo_path)
        .output()
        .await?;

    let mut codec = GitCodec;
    let mut buf = bytes::BytesMut::new();

    codec.encode(
        message::GitMessage::ServiceHeader(message::GitService::ReceivePack),
        &mut buf,
    )?;
    codec.encode(message::GitMessage::Flush, &mut buf)?;
    buf.extend_from_slice(&child.stdout);

    info!(?buf, "Encoded data");

    Ok((
        [
            (
                axum::http::header::CONTENT_TYPE,
                "application/x-git-receive-pack-advertisement",
            ),
            (axum::http::header::CACHE_CONTROL, "no-cache"),
        ],
        buf.freeze(),
    ))
}

/// Generate a response for git-receive-pack using git2
async fn list_refs(
    State(app_state): State<Arc<ServerState>>,
    Path((owner, repo)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
    payload: Bytes,
) -> Result<impl axum::response::IntoResponse> {
    info!("Received data from {}/{}", owner, repo);
    info!("Payload: {:?}", payload);

    // We need to respond with the results for `git receive-pack` to work

    let service = query.get("service").ok_or(error::Error::MissingService)?;
    info!("Service: {}", service);

    // determine the path to the repo
    let repo_path = app_state.repo_path.join(owner).join(repo);
    let repo = git2::Repository::open_bare(&repo_path)?;

    // generate the ref response the client needs
    let mut buf = bytes::BytesMut::new();
    let mut ref_codec = GitCodec;

    let refs = repo.references()?;
    let refs = refs
        .into_iter()
        .filter_map(|r| r.ok())
        .map(|r| r.name().unwrap().to_owned())
        .collect::<Vec<_>>();

    // TODO: capabilities
    ref_codec.encode(
        message::GitMessage::ServiceHeader(message::GitService::ReceivePack),
        &mut buf,
    )?;
    ref_codec.encode(message::GitMessage::Flush, &mut buf)?;

    let capabilities =
        b" capabilities\0report-status delete-refs side-band-64k quiet ofs-delta agent=git/thoenix";
    let empty_sha = b"0000000000000000000000000000000000000000";
    if refs.is_empty() {
        let mut data = bytes::BytesMut::new();
        data.extend_from_slice(empty_sha);
        data.extend_from_slice(capabilities);
        ref_codec.encode(message::GitMessage::Data(data.split().to_vec()), &mut buf)?;

        ref_codec.encode(message::GitMessage::Flush, &mut buf)?;
    } else {
        refs.iter().enumerate().for_each(|(i, r)| {
            let mut data = bytes::BytesMut::from(r.as_bytes());
            // include capabilities with the first ref
            if i == 0 {
                data.extend_from_slice(capabilities);
            }

            ref_codec
                .encode(message::GitMessage::Data(data.to_vec()), &mut buf)
                .unwrap();
        });
    }
    ref_codec
        .encode(message::GitMessage::Flush, &mut buf)
        .unwrap();

    info!(?refs, ?repo_path, ?buf);

    Ok((
        [
            (
                axum::http::header::CONTENT_TYPE,
                "application/x-git-receive-pack-advertisement",
            ),
            (axum::http::header::CACHE_CONTROL, "no-cache"),
        ],
        buf.freeze(),
    ))
}

async fn receive_pack(
    State(_app_state): State<Arc<ServerState>>,
    Path((owner, repo)): Path<(String, String)>,
    _payload: Bytes,
) -> Result<()> {
    info!(%owner, %repo, "Received send-pack data");
    todo!()
}
