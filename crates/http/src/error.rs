#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Hyper(hyper::Error),
}

impl std::convert::From<hyper::Error> for Error {
    fn from(e: hyper::Error) -> Self {
        Error::Hyper(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
