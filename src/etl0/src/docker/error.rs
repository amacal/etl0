use hyper::body::{Bytes, Frame};
use hyper::StatusCode;
use thiserror::Error;

use super::http::DockerResponse;

#[derive(Debug, Error)]
pub enum DockerError {
    #[error("Cannot connected to '{0}', because '{1}'")]
    UnixSocketConnect(String, std::io::Error),

    #[error("Cannot perform handshake to '{0}', because '{1}'")]
    HandshakeFailed(String, hyper::Error),

    #[error("Cannot build HTTP request to '{0}', because '{1}'")]
    BuilderFailed(String, hyper::http::Error),

    #[error("Cannot clean HTTP connection to '{0}', because '{1}'")]
    ConnectionFailed(String, hyper::Error),

    #[error("Cannot join HTTP connection to '{0}', because '{1}'")]
    TokioFailed(String, tokio::task::JoinError),

    #[error("Cannot send HTTP request to '{0}', because '{1}'")]
    RequestFailed(String, hyper::Error),

    #[error("Cannot accept HTTP status code from '{0}', because '{1}'")]
    StatusFailed(String, hyper::http::StatusCode, DockerResponse),

    #[error("Cannot handle HTTP frame from '{0}', because '{1}'")]
    HttpFrameFailed(String, hyper::Error),

    #[error("Cannot recognize HTTP frame from '{0}'")]
    HttpFrameUnrecognized(String, Frame<Bytes>),

    #[error("Cannot receive HTTP response from '{0}', because '{1}'")]
    ResponseFailed(String, hyper::Error),

    #[error("Cannot deserialize JSON payload from '{0:?}', because '{1}'")]
    DeserializationFailed(Option<hyper::http::StatusCode>, serde_json::Error, Bytes),

    #[error("Cannot parse utf8 text, because '{0}'")]
    Utf8ParsingFailed(std::str::Utf8Error),
}

pub type DockerResult<T> = Result<T, DockerError>;

impl DockerError {
    pub(crate) fn raise_unix_socket_connect<T>(socket: &str, error: std::io::Error) -> DockerResult<T> {
        Err(Self::UnixSocketConnect(socket.to_owned(), error))
    }

    pub(crate) fn raise_handshake_failed<T>(socket: &str, error: hyper::Error) -> DockerResult<T> {
        Err(Self::HandshakeFailed(socket.to_owned(), error))
    }

    pub(crate) fn raise_builder_failed<T>(url: &str, error: hyper::http::Error) -> DockerResult<T> {
        Err(Self::BuilderFailed(url.to_owned(), error))
    }

    pub(crate) fn raise_connection_failed<T>(url: &str, error: hyper::Error) -> DockerResult<T> {
        Err(Self::ConnectionFailed(url.to_owned(), error))
    }

    pub(crate) fn raise_tokio_failed<T>(url: &str, error: tokio::task::JoinError) -> DockerResult<T> {
        Err(Self::TokioFailed(url.to_owned(), error))
    }

    pub(crate) fn raise_request_failed<T>(url: &str, error: hyper::Error) -> DockerResult<T> {
        Err(Self::RequestFailed(url.to_owned(), error))
    }

    pub(crate) fn raise_status_failed<T>(status: hyper::http::StatusCode, response: DockerResponse) -> DockerResult<T> {
        Err(Self::StatusFailed(response.url.to_owned(), status, response))
    }

    pub(crate) fn raise_http_frame_failed<T>(url: &str, error: hyper::Error) -> DockerResult<T> {
        Err(Self::HttpFrameFailed(url.to_owned(), error))
    }

    pub(crate) fn raise_http_frame_unrecognized<T>(url: &str, frame: Frame<Bytes>) -> DockerResult<T> {
        Err(Self::HttpFrameUnrecognized(url.to_owned(), frame))
    }

    pub(crate) fn raise_response_failed<T>(url: &str, error: hyper::Error) -> DockerResult<T> {
        Err(Self::ResponseFailed(url.to_owned(), error))
    }

    pub(crate) fn raise_deserialization_failed<T>(
        status: Option<StatusCode>,
        error: serde_json::Error,
        data: Bytes,
    ) -> DockerResult<T> {
        Err(Self::DeserializationFailed(status, error, data))
    }

    pub(crate) fn raise_utf8_parsing_failed<T>(error: std::str::Utf8Error) -> DockerResult<T> {
        Err(Self::Utf8ParsingFailed(error))
    }
}
