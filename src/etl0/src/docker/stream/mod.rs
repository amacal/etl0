mod common;

use std::pin::Pin;
use std::str::from_utf8;
use std::task::{Context, Poll};

use hyper::body::Bytes;

use serde::Deserialize;
use serde_json::from_slice;
use tokio_stream::Stream;

use self::common::{DockerStream, DockerStreamBuffer, DockerStreamHandler};

use super::error::{DockerError, DockerResult};
use super::http::DockerResponse;
use super::ErrorResponse;

#[derive(Debug)]
struct ContainerLogsStreamHandler {}

impl ContainerLogsStreamHandler {
    fn new() -> Self {
        Self {}
    }
}

impl DockerStreamHandler for ContainerLogsStreamHandler {
    type Item = String;

    fn extract(&self, buffer: &mut DockerStreamBuffer) -> Vec<DockerResult<Self::Item>> {
        let mut current: usize = 0;
        let mut broken = false;
        let mut result = Vec::new();

        let data = buffer.as_ref();
        let length = data.len();

        while !broken && current < length {
            if current + 8 > length {
                break;
            }

            let size = u32::from_be_bytes([
                data[current + 4],
                data[current + 5],
                data[current + 6],
                data[current + 7],
            ]) as usize;

            let start = current + 8;
            let end = start + size;

            if end > length {
                break;
            }

            let message = match from_utf8(&data[start..end]) {
                Err(error) => DockerError::raise_utf8_parsing_failed(error),
                Ok(value) => Ok(value.to_string()),
            };

            if let Err(_) = message {
                broken = true;
            }

            result.push(message);
            current = end;

            if broken {
                break;
            }
        }

        if current > 0 {
            buffer.consume(current);
        }

        result
    }
}

#[derive(Debug)]
pub struct ContainerLogsStream {
    inner: DockerStream<ContainerLogsStreamHandler>,
}

impl ContainerLogsStream {
    pub fn from(response: DockerResponse) -> Self {
        Self {
            inner: DockerStream::from(ContainerLogsStreamHandler::new(), response),
        }
    }
}

impl Stream for ContainerLogsStream {
    type Item = DockerResult<String>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let self_mut = self.get_mut();
        let pointer = &mut self_mut.inner;
        let pin = Pin::new(pointer);

        pin.poll_next(cx)
    }
}

#[derive(Debug)]
struct ImageCreateStreamHandler {}

impl ImageCreateStreamHandler {
    fn new() -> Self {
        Self {}
    }
}

impl DockerStreamHandler for ImageCreateStreamHandler {
    type Item = ImageCreateStreamLine;

    fn extract(&self, buffer: &mut DockerStreamBuffer) -> Vec<DockerResult<Self::Item>> {
        let mut current: usize = 0;
        let mut result: Vec<DockerResult<ImageCreateStreamItem>> = Vec::new();

        let data = buffer.as_ref();
        let length = data.len();

        while current < length {
            if current + 2 > length {
                break;
            }

            for i in current..length - 1 {
                if data[i] == 0x0d && data[i + 1] == 0x0a {
                    let item: DockerResult<ImageCreateStreamItem> = {
                        let data: &[u8] = &data[current..i];
                        let data: Bytes = Bytes::from(data.to_vec());

                        match from_slice(&data) {
                            Ok(value) => Ok(value),
                            Err(error) => DockerError::raise_deserialization_failed(None, error, data),
                        }
                    };

                    result.push(item);
                    current = i + 2;

                    continue;
                }
            }

            break;
        }

        if current > 0 {
            buffer.consume(current);
        }

        result.into_iter().map(ImageCreateStreamLine::from).collect()
    }
}

#[derive(Debug)]
pub struct ImageCreateStream {
    inner: DockerStream<ImageCreateStreamHandler>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageCreateStreamProgress {
    pub current: Option<u64>,
    pub total: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageCreateStreamItem {
    pub status: Option<String>,
    pub id: Option<String>,
    pub error: Option<String>,
    #[serde(rename = "errorDetail")]
    pub error_detail: Option<ErrorResponse>,
    pub progress: Option<String>,
    #[serde(rename = "progressDetail")]
    pub progress_detail: Option<ImageCreateStreamProgress>,
}

#[derive(Debug)]
pub struct ImageCreateStreamLineStatus {
    pub id: String,
    pub status: String,
}

#[derive(Debug)]
pub struct ImageCreateStreamLineInfo {
    pub status: String,
}

#[derive(Debug)]
pub struct ImageCreateStreamLineProgress {
    pub id: String,
    pub status: String,
    pub info: String,
    pub total: u64,
    pub current: u64,
}

#[derive(Debug)]
pub struct ImageCreateStreamLineError {
    pub message: String,
    pub detail: String,
}

#[derive(Debug)]
pub enum ImageCreateStreamLine {
    Status(ImageCreateStreamLineStatus),
    Info(ImageCreateStreamLineInfo),
    Progress(ImageCreateStreamLineProgress),
    Error(ImageCreateStreamLineError),
    Raw(ImageCreateStreamItem),
}

impl ImageCreateStreamLine {
    fn from(item: DockerResult<ImageCreateStreamItem>) -> DockerResult<Self> {
        let item = match item {
            Ok(value) => value,
            Err(error) => return Err(error),
        };

        if let (Some(message), Some(detail)) = (&item.error, &item.error_detail) {
            return Ok(ImageCreateStreamLine::Error(ImageCreateStreamLineError {
                message: message.clone(),
                detail: detail.message.clone(),
            }));
        }

        if let (
            Some(id),
            Some(status),
            Some(progress),
            Some(ImageCreateStreamProgress {
                total: Some(total),
                current: Some(current),
            }),
        ) = (&item.id, &item.status, &item.progress, &item.progress_detail)
        {
            return Ok(ImageCreateStreamLine::Progress(ImageCreateStreamLineProgress {
                id: id.clone(),
                status: status.clone(),
                info: progress.clone(),
                total: total.clone(),
                current: current.clone(),
            }));
        }

        if let (Some(id), Some(status)) = (&item.id, &item.status) {
            return Ok(ImageCreateStreamLine::Status(ImageCreateStreamLineStatus {
                id: id.clone(),
                status: status.clone(),
            }));
        }

        if let Some(status) = &item.status {
            return Ok(ImageCreateStreamLine::Info(ImageCreateStreamLineInfo {
                status: status.clone(),
            }));
        }

        Ok(ImageCreateStreamLine::Raw(item))
    }
}

impl ImageCreateStream {
    pub fn from(response: DockerResponse) -> Self {
        Self {
            inner: DockerStream::from(ImageCreateStreamHandler::new(), response),
        }
    }
}

impl Stream for ImageCreateStream {
    type Item = DockerResult<ImageCreateStreamLine>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let self_mut = self.get_mut();
        let pointer = &mut self_mut.inner;
        let pin = Pin::new(pointer);

        pin.poll_next(cx)
    }
}
