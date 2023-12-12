#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("state not found")]
    NotFound,
    #[error("state is locked")]
    StateLocked,
}

pub type Result<T> = std::result::Result<T, Error>;
