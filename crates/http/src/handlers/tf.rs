use crate::{
    error::{Error, Result},
    ServerState,
};
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use std::sync::Arc;
use thoenix_tofu::{TerraformLock, TerraformLockQuery, TerraformStateProvider};
use tracing::info;

pub async fn get_tf_state(
    Path(id): Path<String>,
    State(app_state): State<Arc<ServerState>>,
) -> Result<impl IntoResponse> {
    info!("Received request for tf state {}", id);

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
