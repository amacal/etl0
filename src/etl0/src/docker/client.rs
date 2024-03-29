use http_body_util::Full;
use hyper::body::Bytes;
use serde_json::{json, Value};

use super::error::{DockerError, DockerResult};
use super::http::DockerConnection;
use super::stream::{ContainerLogsStream, ImageCreateStream};
use super::tar::TarBody;
use super::types::*;
use crate::tar::{TarArchive, TarStream};

#[derive(Debug)]
pub struct DockerClient {
    socket: String,
}

impl DockerClient {
    pub fn open(socket: &str) -> Self {
        Self {
            socket: socket.to_owned(),
        }
    }

    pub async fn containers_list(&self) -> DockerResult<ContainerList> {
        let connection: DockerConnection<Full<Bytes>> = DockerConnection::open(&self.socket).await?;

        match connection.get("/v1.42/containers/json?all=true").await {
            Ok(response) => match response.into_json().await {
                Ok(value) => Ok(ContainerList::Succeeded(value)),
                Err(error) => Err(error),
            },
            Err(error) => match error {
                DockerError::StatusFailed(url, status, response) => match status.as_u16() {
                    400 => Ok(ContainerList::BadParameter(response.into_error().await?)),
                    500 => Ok(ContainerList::ServerError(response.into_error().await?)),
                    _ => Err(DockerError::StatusFailed(url, status, response)),
                },
                error => Err(error),
            },
        }
    }

    pub async fn containers_create(&self, spec: &ContainerCreateSpec<'_>) -> DockerResult<ContainerCreate> {
        let url: String = format!("/v1.42/containers/create");
        let payload: Value = json!({"Image": spec.image, "Cmd": spec.command});
        let connection: DockerConnection<Full<Bytes>> = DockerConnection::open(&self.socket).await?;

        match connection.post(&url, Some(payload)).await {
            Ok(response) => match response.into_json().await {
                Ok(value) => Ok(ContainerCreate::Succeeded(value)),
                Err(error) => Err(error),
            },
            Err(error) => match error {
                DockerError::StatusFailed(url, status, response) => match status.as_u16() {
                    400 => Ok(ContainerCreate::BadParameter(response.into_error().await?)),
                    404 => Ok(ContainerCreate::NoSuchImage(response.into_error().await?)),
                    409 => Ok(ContainerCreate::Conflict(response.into_error().await?)),
                    500 => Ok(ContainerCreate::ServerError(response.into_error().await?)),
                    _ => Err(DockerError::StatusFailed(url, status, response)),
                },
                error => Err(error),
            },
        }
    }

    pub async fn containers_start(&self, id: &str) -> DockerResult<ContainerStart> {
        let url: String = format!("/v1.42/containers/{id}/start");
        let connection: DockerConnection<Full<Bytes>> = DockerConnection::open(&self.socket).await?;

        match connection.post(&url, None).await {
            Ok(response) => match response.into_bytes().await {
                Ok(_) => Ok(ContainerStart::Succeeded),
                Err(error) => Err(error),
            },
            Err(error) => match error {
                DockerError::StatusFailed(url, status, response) => match status.as_u16() {
                    304 => Ok(ContainerStart::AlreadyStarted),
                    404 => Ok(ContainerStart::NoSuchContainer(response.into_error().await?)),
                    500 => Ok(ContainerStart::ServerError(response.into_error().await?)),
                    _ => Err(DockerError::StatusFailed(url, status, response)),
                },
                error => Err(error),
            },
        }
    }

    pub async fn containers_stop(&self, id: &str) -> DockerResult<ContainerStop> {
        let url: String = format!("/v1.42/containers/{id}/stop");
        let connection: DockerConnection<Full<Bytes>> = DockerConnection::open(&self.socket).await?;

        match connection.post(&url, None).await {
            Ok(response) => match response.into_bytes().await {
                Ok(_) => Ok(ContainerStop::Succeeded),
                Err(error) => Err(error),
            },
            Err(error) => match error {
                DockerError::StatusFailed(url, status, response) => match status.as_u16() {
                    304 => Ok(ContainerStop::AlreadyStopped),
                    404 => Ok(ContainerStop::NoSuchContainer(response.into_error().await?)),
                    500 => Ok(ContainerStop::ServerError(response.into_error().await?)),
                    _ => Err(DockerError::StatusFailed(url, status, response)),
                },
                error => Err(error),
            },
        }
    }

    pub async fn containers_wait(&self, id: &str) -> DockerResult<ContainerWait> {
        let url: String = format!("/v1.42/containers/{id}/wait");
        let connection: DockerConnection<Full<Bytes>> = DockerConnection::open(&self.socket).await?;

        match connection.post(&url, None).await {
            Ok(response) => match response.into_json().await {
                Ok(value) => Ok(ContainerWait::Succeeded(value)),
                Err(error) => Err(error),
            },
            Err(error) => match error {
                DockerError::StatusFailed(url, status, response) => match status.as_u16() {
                    400 => Ok(ContainerWait::BadParameter(response.into_error().await?)),
                    404 => Ok(ContainerWait::NoSuchContainer(response.into_error().await?)),
                    500 => Ok(ContainerWait::ServerError(response.into_error().await?)),
                    _ => Err(DockerError::StatusFailed(url, status, response)),
                },
                error => Err(error),
            },
        }
    }

    pub async fn containers_remove(&self, id: &str) -> DockerResult<ContainerRemove> {
        let url: String = format!("/v1.42/containers/{id}");
        let connection: DockerConnection<Full<Bytes>> = DockerConnection::open(&self.socket).await?;

        match connection.delete(&url).await {
            Ok(response) => match response.into_bytes().await {
                Ok(_) => Ok(ContainerRemove::Succeeded),
                Err(error) => Err(error),
            },
            Err(error) => match error {
                DockerError::StatusFailed(url, status, response) => match status.as_u16() {
                    400 => Ok(ContainerRemove::BadParameter(response.into_error().await?)),
                    404 => Ok(ContainerRemove::NoSuchContainer(response.into_error().await?)),
                    409 => Ok(ContainerRemove::Conflict(response.into_error().await?)),
                    500 => Ok(ContainerRemove::ServerError(response.into_error().await?)),
                    _ => Err(DockerError::StatusFailed(url, status, response)),
                },
                error => Err(error),
            },
        }
    }

    pub async fn containers_logs(&self, id: &str) -> DockerResult<ContainerLogs> {
        let url: String = format!("/v1.42/containers/{id}/logs?stdout=true");
        let connection: DockerConnection<Full<Bytes>> = DockerConnection::open(&self.socket).await?;

        match connection.get(&url).await {
            Ok(response) => Ok(ContainerLogs::Succeeded(ContainerLogsStream::from(response))),
            Err(error) => match error {
                DockerError::StatusFailed(url, status, response) => match status.as_u16() {
                    404 => Ok(ContainerLogs::NoSuchContainer(response.into_error().await?)),
                    500 => Ok(ContainerLogs::ServerError(response.into_error().await?)),
                    _ => Err(DockerError::StatusFailed(url, status, response)),
                },
                error => Err(error),
            },
        }
    }

    pub async fn containers_attach(&self, id: &str) -> DockerResult<ContainerAttach> {
        let url: String = format!("/v1.42/containers/{id}/attach?logs=true&stream=true&stdout=true&stderr=true");
        let connection: DockerConnection<Full<Bytes>> = DockerConnection::open(&self.socket).await?;

        match connection.post(&url, None).await {
            Ok(response) => Ok(ContainerAttach::Succeeded(ContainerLogsStream::from(response))),
            Err(error) => match error {
                DockerError::StatusFailed(url, status, response) => match status.as_u16() {
                    400 => Ok(ContainerAttach::BadParameter(response.into_error().await?)),
                    404 => Ok(ContainerAttach::NoSuchContainer(response.into_error().await?)),
                    500 => Ok(ContainerAttach::ServerError(response.into_error().await?)),
                    _ => Err(DockerError::StatusFailed(url, status, response)),
                },
                error => Err(error),
            },
        }
    }

    pub async fn container_upload(&self, id: &str, path: &str, archive: TarArchive) -> DockerResult<ContainerUpload> {
        let url: String = format!("/v1.42/containers/{id}/archive?path={path}");
        let connection: DockerConnection<TarBody> = DockerConnection::open(&self.socket).await?;

        let stream: TarStream = archive.into_stream(64 * 1024);
        let data: TarBody = TarBody::from(stream);

        match connection.put(&url, data).await {
            Ok(response) => match response.into_bytes().await {
                Ok(_) => Ok(ContainerUpload::Succeeded),
                Err(error) => Err(error),
            },
            Err(error) => match error {
                DockerError::StatusFailed(url, status, response) => match status.as_u16() {
                    400 => Ok(ContainerUpload::BadParameter(response.into_error().await?)),
                    403 => Ok(ContainerUpload::PermissionDenied(response.into_error().await?)),
                    404 => Ok(ContainerUpload::NoSuchContainer(response.into_error().await?)),
                    500 => Ok(ContainerUpload::ServerError(response.into_error().await?)),
                    _ => Err(DockerError::StatusFailed(url, status, response)),
                },
                error => Err(error),
            },
        }
    }

    pub async fn images_create(&self) -> DockerResult<ImageCreate> {
        let url: String = format!("/v1.42/images/create?fromImage=python:3.12");
        let connection: DockerConnection<Full<Bytes>> = DockerConnection::open(&self.socket).await?;

        match connection.post(&url, None).await {
            Ok(response) => Ok(ImageCreate::Succeeded(ImageCreateStream::from(response))),
            Err(error) => match error {
                DockerError::StatusFailed(url, status, response) => match status.as_u16() {
                    404 => Ok(ImageCreate::NoReadAccess(response.into_error().await?)),
                    500 => Ok(ImageCreate::ServerError(response.into_error().await?)),
                    _ => Err(DockerError::StatusFailed(url, status, response)),
                },
                error => Err(error),
            },
        }
    }
}
