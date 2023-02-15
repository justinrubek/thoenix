#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Hyper(hyper::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Git(#[from] git2::Error),

    #[error("Missing service")]
    MissingService,
    #[error("Unable to parse length bytes")]
    ParseLengthBytes,
}

impl std::convert::From<hyper::Error> for Error {
    fn from(e: hyper::Error) -> Self {
        Error::Hyper(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

impl axum::response::IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let status = match self {
            Error::Hyper(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Error::Io(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Error::Git(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,

            Error::MissingService => axum::http::StatusCode::BAD_REQUEST,
            Error::ParseLengthBytes => axum::http::StatusCode::BAD_REQUEST,
        };

        (status, self.to_string()).into_response()
    }
}
