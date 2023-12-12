use crate::{
    error::{Error, Result},
    ServerState,
};
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct TerraformLock {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "Operation")]
    pub operation: String,
    #[serde(rename = "Info")]
    pub info: String,
    #[serde(rename = "Who")]
    pub who: String,
    #[serde(rename = "Version")]
    pub version: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TerraformLockQuery {
    #[serde(rename = "ID")]
    pub id: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TerraformState {
    lock: Option<TerraformLock>,
    pub data: String,
}

impl TerraformState {
    pub fn is_locked(&self) -> bool {
        self.lock.is_some()
    }

    pub fn check_lock(&self, id: &str) -> Result<()> {
        if self.lock.as_ref().is_some_and(|l| l.id != id) {
            return Err(Error::StateLocked);
        }

        Ok(())
    }

    pub fn lock(&mut self, lock: TerraformLock) -> Result<()> {
        if self.is_locked() {
            return Err(Error::StateLocked);
        }

        self.lock = Some(lock);

        Ok(())
    }

    pub fn unlock(&mut self, id: &TerraformLock) -> Result<()> {
        if self.lock.as_ref().is_some_and(|l| l.id != id.id) {
            return Err(Error::StateLocked);
        }

        self.lock = None;

        Ok(())
    }
}

pub async fn get_tf_state(
    Path(id): Path<String>,
    State(app_state): State<Arc<ServerState>>,
) -> Result<impl IntoResponse> {
    info!("Received request for tf state {}", id);

    // let state = app_state.tf_state.get_state(&id).await?.ok_or(Error::NotFound)?;
    // create the state if it doesn't exist
    let mut state = app_state.tf_state.lock().await;
    let state = match state.get_state(&id).await? {
        Some(state) => state,
        None => {
            state.create_state(id.clone()).await?;
            state.get_state(&id).await?.ok_or(Error::NotFound)?
        }
    };

    Ok((axum::http::StatusCode::OK, state.data.clone()))
}

pub async fn update_tf_state(
    Path(id): Path<String>,
    State(app_state): State<Arc<ServerState>>,
    Query(lock_query): Query<TerraformLockQuery>,
    payload: String,
) -> Result<impl IntoResponse> {
    info!("Received request to update tf state {}", id);

    let mut state = app_state.tf_state.lock().await;
    state.update_state(&id, &lock_query.id, payload).await?;

    Ok(axum::http::StatusCode::OK)
}

pub async fn lock_tf_state(
    Path(id): Path<String>,
    State(app_state): State<Arc<ServerState>>,
    // Json(body): Json<TerraformLock>,
    Json(body): Json<TerraformLock>,
) -> Result<impl IntoResponse> {
    info!(?body, "Received request to lock tf state {}", id);

    let mut state = app_state.tf_state.lock().await;
    state.lock_state(&id, body).await?;

    Ok(axum::http::StatusCode::OK)
}

pub async fn unlock_tf_state(
    Path(id): Path<String>,
    State(app_state): State<Arc<ServerState>>,
    Json(body): Json<TerraformLock>,
) -> Result<impl IntoResponse> {
    info!(?body, "Received request to unlock tf state {}", id);

    let mut state = app_state.tf_state.lock().await;
    state.unlock_state(&id, &body).await?;

    Ok(axum::http::StatusCode::OK)
}
