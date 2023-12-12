use axum::{
    extract::MatchedPath,
    http::Request,
    routing::{get, post, put},
    Router,
};
#[allow(unused_imports)]
use handlers::{
    git::{list_refs, list_refs_child, receive_pack},
    tf::{
        get_tf_state, lock_tf_state, unlock_tf_state, update_tf_state, TerraformLock,
        TerraformState,
    },
};
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
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

#[derive(Debug, Default)]
pub struct InMemoryState {
    pub tf_state: std::collections::HashMap<String, TerraformState>,
}

impl InMemoryState {
    pub fn new() -> Self {
        Default::default()
    }

    pub async fn create_state(&mut self, id: String) -> Result<()> {
        self.tf_state.insert(id, TerraformState::default());

        Ok(())
    }

    pub async fn get_state(&self, id: &str) -> Result<Option<TerraformState>> {
        let state = self.tf_state.get(id).cloned();

        Ok(state)
    }

    pub async fn expect_not_locked(&self, id: &str) -> Result<()> {
        let state = self.tf_state.get(id).ok_or(error::Error::NotFound)?;

        if state.is_locked() {
            return Err(error::Error::StateLocked);
        }

        Ok(())
    }

    pub async fn update_state(&mut self, id: &str, lock_id: &str, data: String) -> Result<()> {
        // Ensure that the state is not locked
        let state = self.tf_state.get_mut(id).ok_or(error::Error::NotFound)?;
        state.check_lock(lock_id)?;

        state.data = data;

        Ok(())
    }

    pub async fn lock_state(&mut self, id: &str, lock: TerraformLock) -> Result<()> {
        let state = self.tf_state.get_mut(id).ok_or(error::Error::NotFound)?;

        state.lock(lock)?;

        Ok(())
    }

    pub async fn unlock_state(&mut self, id: &str, lock: &TerraformLock) -> Result<()> {
        let state = self.tf_state.get_mut(id).ok_or(error::Error::NotFound)?;

        state.unlock(lock)?;

        Ok(())
    }
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
                span.record("request_id", &tracing::field::display(id));
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
