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

    // Application specific errors
    #[error("no data directory specified")]
    NoDataDir,

    #[error(transparent)]
    SshError(#[from] thoenix_ssh::error::Error),
    #[error(transparent)]
    HttpError(#[from] thoenix_http::error::Error),
}

pub type AppResult<T> = Result<T, AppError>;
