use axum::{routing::get, Router};
use std::{net::SocketAddr, path::PathBuf};

pub mod error;

use error::Result;

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

        let app = Router::new().route("/", get(root)).layer(cors);

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
