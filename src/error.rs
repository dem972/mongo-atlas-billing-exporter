//use serde_json::error::Error as SerdeError;
use axum::{
    body::{self},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use std::fmt;

#[derive(Debug)]
pub enum Error {
    Forbidden,
    Unauthorized,
    NotFound,
    UnknownCode,
    UnexpectedCode,
    MissingHeader,
    Hyper(hyper::Error),
    Digest(digest_auth::Error),
    SerdeJson(serde_json::Error),
    InvalidHeaderValue(hyper::header::InvalidHeaderValue),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Forbidden => f.write_str("{\"error\": \"Status: Forbidden\"}"),
            Error::UnknownCode => f.write_str("{\"error\": \"Caught bad status code\"}"),
            Error::Unauthorized => f.write_str("{\"error\": \"Status: Unauthorized\"}"),
            Error::NotFound => f.write_str("{\"error\": \"Status: Not found\"}"),
            Error::UnexpectedCode => {
                f.write_str("{\"error\": \"Unexpected status code received\"}")
            }
            Error::MissingHeader => {
                f.write_str("{\"error\": \"Missing expected response header\"}")
            }
            Error::Hyper(ref err) => write!(f, "{{\"error\": \"{err}\"}}"),
            Error::SerdeJson(ref err) => write!(f, "{{\"error\": \"{err}\"}}"),
            Error::Digest(ref err) => write!(f, "{{\"error\": \"{err}\"}}"),
            Error::InvalidHeaderValue(ref err) => write!(f, "{{\"error\": \"{err}\"}}"),
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let payload = self.to_string();
        let body = body::boxed(body::Full::from(payload));

        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(body)
            .unwrap()
    }
}

impl From<hyper::Error> for Error {
    fn from(err: hyper::Error) -> Error {
        Error::Hyper(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Error {
        Error::SerdeJson(err)
    }
}

impl From<digest_auth::Error> for Error {
    fn from(err: digest_auth::Error) -> Error {
        Error::Digest(err)
    }
}

impl From<hyper::header::InvalidHeaderValue> for Error {
    fn from(err: hyper::header::InvalidHeaderValue) -> Error {
        Error::InvalidHeaderValue(err)
    }
}
