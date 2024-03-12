use std::path::Path;

use hyper::body::{Bytes, Incoming};
use hyper::client::conn::http1::{handshake, SendRequest};
use hyper::{Request, Response, StatusCode};

use http_body_util::{BodyExt, Full};
use hyper_util::rt::TokioIo;
use serde_json::{from_slice, Value};

use tokio::net::UnixStream;
use tokio::spawn;
use tokio::task::JoinHandle;

use super::error::{DockerError, DockerResult};
use super::types::ErrorResponse;

#[derive(Debug)]
pub(crate) struct DockerResponse {
    pub(crate) url: String,
    pub(crate) inner: Response<Incoming>,
    pub(crate) connection: JoinHandle<Result<(), hyper::Error>>,
}

impl DockerResponse {
    fn new(url: &str, response: Response<Incoming>, connection: JoinHandle<Result<(), hyper::Error>>) -> Self {
        Self {
            url: url.to_owned(),
            inner: response,
            connection: connection,
        }
    }

    pub async fn into_bytes(self) -> DockerResult<Bytes> {
        let data: Bytes = match self.inner.collect().await {
            Err(error) => return DockerError::raise_response_failed(&self.url, error),
            Ok(value) => value.to_bytes(),
        };

        match self.connection.await {
            Err(error) => return DockerError::raise_tokio_failed(&self.url, error),
            Ok(Err(error)) => return DockerError::raise_connection_failed(&self.url, error),
            _ => (),
        }

        Ok(data)
    }

    pub async fn into_json<T>(self) -> DockerResult<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let status: StatusCode = self.inner.status();
        let data: Bytes = self.into_bytes().await?;

        match from_slice(data.as_ref()) {
            Err(error) => DockerError::raise_deserialization_failed(Some(status), error, data),
            Ok(value) => Ok(value),
        }
    }

    pub async fn into_error(self) -> DockerResult<ErrorResponse> {
        self.into_json().await
    }
}

pub struct DockerConnection {
    sender: SendRequest<Full<Bytes>>,
    connection: JoinHandle<Result<(), hyper::Error>>,
}

impl DockerConnection {
    pub async fn open(socket: &str) -> DockerResult<Self> {
        let stream: TokioIo<UnixStream> = match UnixStream::connect(Path::new(socket)).await {
            Err(error) => return DockerError::raise_unix_socket_connect(socket, error),
            Ok(stream) => TokioIo::new(stream),
        };

        let docker: DockerConnection = match handshake(stream).await {
            Err(error) => return DockerError::raise_handshake_failed(socket, error),
            Ok((sender, connection)) => Self {
                sender: sender,
                connection: spawn(async move { connection.await }),
            },
        };

        Ok(docker)
    }

    async fn execute(mut self, url: &str, request: Request<Full<Bytes>>) -> DockerResult<DockerResponse> {
        let response: Response<Incoming> = match self.sender.send_request(request).await {
            Err(error) => return DockerError::raise_request_failed(url, error),
            Ok(value) => value,
        };

        let status: StatusCode = response.status();
        let response: DockerResponse = DockerResponse::new(url, response, self.connection);

        if !status.is_success() {
            return DockerError::raise_status_failed(status, response);
        }

        Ok(response)
    }

    pub async fn get(self, url: &str) -> DockerResult<DockerResponse> {
        let request = Request::builder()
            .uri(url)
            .method("GET")
            .header("Host", "localhost")
            .body(Full::new(Bytes::new()));

        let request: Request<Full<Bytes>> = match request {
            Err(error) => return DockerError::raise_builder_failed(url, error),
            Ok(value) => value,
        };

        self.execute(url, request).await
    }

    pub async fn post(self, url: &str, body: Option<Value>) -> DockerResult<DockerResponse> {
        let request = Request::builder()
            .uri(url)
            .method("POST")
            .header("Host", "localhost")
            .header("Content-Type", "application/json");

        let request = match body {
            None => request.body(Full::new(Bytes::new())),
            Some(value) => request.body(Full::new(Bytes::from(value.to_string()))),
        };

        let request: Request<Full<Bytes>> = match request {
            Err(error) => return DockerError::raise_builder_failed(url, error),
            Ok(value) => value,
        };

        self.execute(url, request).await
    }

    pub async fn delete(self, url: &str) -> DockerResult<DockerResponse> {
        let request = Request::builder()
            .uri(url)
            .method("DELETE")
            .header("Host", "localhost")
            .body(Full::new(Bytes::new()));

        let request: Request<Full<Bytes>> = match request {
            Err(error) => return DockerError::raise_builder_failed(url, error),
            Ok(value) => value,
        };

        self.execute(url, request).await
    }
}
