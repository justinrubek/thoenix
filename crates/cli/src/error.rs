use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
    #[error(transparent)]
    Russh(#[from] russh::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    FromUtf8(#[from] std::string::FromUtf8Error),
    #[error(transparent)]
    Ed25519(#[from] ed25519_dalek::ed25519::Error),
    #[error(transparent)]
    GitPackDataInit(#[from] git_pack::data::init::Error),
    #[error(transparent)]
    Utf8(#[from] std::str::Utf8Error),

    #[error(transparent)]
    SshError(#[from] thoenix_ssh::error::Error),
    #[error(transparent)]
    HttpError(#[from] thoenix_http::error::Error),

    #[error("terraform error: {0}")]
    TerraformError(i32),
    #[error("git repo error: {0}")]
    GitRepo(#[from] git2::Error),
    #[error("failed to execute nix: {0}")]
    Nix(i32),
}

pub type AppResult<T> = Result<T, AppError>;
