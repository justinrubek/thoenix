#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Hyper(#[from] hyper::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Git(#[from] git2::Error),
    #[error(transparent)]
    Tofu(#[from] thoenix_tofu::error::Error),
    #[error(transparent)]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("Missing service")]
    MissingService,
    #[error("Unable to parse length bytes")]
    ParseLengthBytes,
    #[error("not found")]
    NotFound,
    #[error("state is locked")]
    StateLocked,
}

pub type Result<T> = std::result::Result<T, Error>;

impl axum::response::IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let status = match self {
            Error::Hyper(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Error::Io(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Error::Git(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Error::Tofu(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Error::Utf8(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,

            Error::MissingService => axum::http::StatusCode::BAD_REQUEST,
            Error::ParseLengthBytes => axum::http::StatusCode::BAD_REQUEST,
            Error::NotFound => axum::http::StatusCode::NOT_FOUND,
            Error::StateLocked => axum::http::StatusCode::CONFLICT,
        };

        (status, self.to_string()).into_response()
    }
}
