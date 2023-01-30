use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
    #[error(transparent)]
    Russh(#[from] russh::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),

    // Application specific errors
    #[error("no data directory specified")]
    NoDataDir,
}

pub type AppResult<T> = Result<T, AppError>;
