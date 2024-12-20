use axum::{
    extract::MatchedPath,
    http::Request,
    routing::{get, post, put},
    Router,
};
#[allow(unused_imports)]
use handlers::{
    git::{list_refs, list_refs_child, receive_pack},
    tf::{get_tf_state, lock_tf_state, unlock_tf_state, update_tf_state},
};
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use thoenix_tofu::InMemoryState;
use tracing::{info_span, Span};

pub mod codec;
pub mod error;
pub mod handlers;
pub mod message;

use error::Result;
use message::GitCodec;

pub struct ServerState {
    pub repo_path: PathBuf,

    pub tf_state: tokio::sync::Mutex<InMemoryState>,
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
        let tracing_layer = tower_http::trace::TraceLayer::new_for_http()
            .make_span_with(|request: &Request<_>| {
                let matched_path = request
                    .extensions()
                    .get::<MatchedPath>()
                    .map(MatchedPath::as_str);

                info_span!(
                    "http_request",
                    method = ?request.method(),
                    matched_path,
                    request_id = tracing::field::Empty,
                )
            })
            .on_request(|_request: &Request<_>, span: &Span| {
                let id = "TODO: request ID";
                span.record("request_id", tracing::field::display(id));
            });

        let addr = SocketAddr::from(([0, 0, 0, 0], port));

        let app_state = Arc::new(ServerState {
            repo_path: self.data_dir.clone(),
            tf_state: tokio::sync::Mutex::new(InMemoryState::new()),
        });

        // TODO: Attempt to implement the `git-receive-pack` route but without calling into
        // git. This could potentially be done using `git_pack::data::entry::Entry` to parse
        // the file and `git_odn` to write? The method to implement is not clear yet.
        let app = Router::new()
            // .route("/configs/:owner/:repo.git/info/refs", get(list_refs_child))
            .route("/configs/:owner/:repo.git/info/refs", get(list_refs))
            .route(
                "/configs/:owner/:repo.git/git-receive-pack",
                post(receive_pack),
            )
            .route("/tf/state/:id", get(get_tf_state).post(update_tf_state))
            .route("/tf/lock/:id", put(lock_tf_state).delete(unlock_tf_state))
            .with_state(app_state)
            .layer(tracing_layer)
            .layer(cors)
            .fallback(get(|| async { "Hello, World!" }));

        println!("Listening on {addr}");
        axum::Server::bind(&addr)
            .serve(app.into_make_service())
            .await?;

        Ok(())
    }
}
