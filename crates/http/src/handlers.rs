use crate::{
    error::{self, Error, Result},
    message::{self},
    GitCodec, ServerState,
};
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
};
use std::{collections::HashMap, sync::Arc};
use tokio::io::AsyncWriteExt;
use tokio_util::codec::Encoder;
use tracing::{debug, info};

pub(crate) mod tf;

// Generate a response for git-receive-pack using the `git` executable
#[allow(dead_code)]
pub(crate) async fn list_refs_child(
    State(app_state): State<Arc<ServerState>>,
    Path((owner, repo)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<impl axum::response::IntoResponse> {
    info!("Received data for {}/{}", owner, repo);

    let service = query.get("service").ok_or(error::Error::MissingService)?;
    info!(?service);

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

    // With the header finished, just return all the data from the child process
    buf.extend_from_slice(&child.stdout);

    debug!(?buf, "Encoded data");

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
#[allow(dead_code)]
pub(crate) async fn list_refs(
    State(app_state): State<Arc<ServerState>>,
    Path((owner, repo)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<impl axum::response::IntoResponse> {
    debug!("Received data for {}/{}", owner, repo);

    let service = query.get("service").ok_or(error::Error::MissingService)?;
    debug!(?service);

    // determine the path to the repo
    let repo_path = app_state.repo_path.join(owner).join(repo);
    let repo = git2::Repository::open_bare(&repo_path)?;

    // generate the ref response the client needs
    let mut buf = bytes::BytesMut::new();
    let mut codec = GitCodec;

    let refs = repo.references()?;
    // format the references how git wants them - the reference hash and the reference name
    let mut refs = refs
        .into_iter()
        .filter_map(|r| r.ok())
        .map(|r| {
            let target = r.target().unwrap().to_string();
            let name = r.name().unwrap();

            let mut buf = bytes::BytesMut::new();
            buf.extend_from_slice(target.as_ref());
            buf.extend_from_slice(b" ");
            buf.extend_from_slice(name.as_ref());

            buf
        })
        .collect::<Vec<_>>();

    codec.encode(
        message::GitMessage::ServiceHeader(message::GitService::ReceivePack),
        &mut buf,
    )?;
    codec.encode(message::GitMessage::Flush, &mut buf)?;

    // We need to attach capabilities to the first ref
    let capabilities =
        b"\0report-status report-status-v2 delete-refs side-band-64k quiet atomic ofs-delta object-format=sha1 agent=git/thoenix";
    // If there are no refs, we need to add a dummy ref representing a "null sha1"
    let empty_sha = b"0000000000000000000000000000000000000000";

    if refs.is_empty() {
        let mut data = bytes::BytesMut::new();
        data.extend_from_slice(empty_sha);
        data.extend_from_slice(capabilities);
        codec.encode(message::GitMessage::Data(data.split().to_vec()), &mut buf)?;
    } else {
        refs.iter_mut().enumerate().try_for_each(|(i, r)| {
            if i == 0 {
                r.extend_from_slice(capabilities);
            }

            codec.encode(message::GitMessage::Data(r.to_vec()), &mut buf)?;

            Ok::<(), Error>(())
        })?;
    }
    codec.encode(message::GitMessage::Flush, &mut buf).unwrap();

    debug!(?refs, ?repo_path, ?buf);

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

pub(crate) async fn receive_pack(
    State(app_state): State<Arc<ServerState>>,
    Path((owner, repo)): Path<(String, String)>,
    payload: Bytes,
) -> Result<Bytes> {
    info!(%owner, %repo, "Received send-pack data");
    let len = payload.len();
    info!(?len);

    let repo_path = app_state.repo_path.join(owner).join(repo);

    // invoke git-receive-pack and pipe the payload to it
    let mut child = tokio::process::Command::new("git")
        .arg("receive-pack")
        .arg(&repo_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(&payload).await?;

    let output = child.wait_with_output().await?;

    info!(?output);

    Ok(bytes::Bytes::from(output.stdout))
}
