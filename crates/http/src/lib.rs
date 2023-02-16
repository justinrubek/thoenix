use axum::{
    routing::{get, post},
    Router,
};
use std::{net::SocketAddr, path::PathBuf, sync::Arc};

pub mod codec;
pub mod error;
pub mod handlers;
pub mod message;

use error::Result;
#[allow(unused_imports)]
use handlers::{list_refs, list_refs_child, receive_pack};
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
            .with_state(app_state)
            .layer(cors);

        println!("Listening on {addr}");
        axum::Server::bind(&addr)
            .serve(app.into_make_service())
            .await?;

        Ok(())
    }
}
