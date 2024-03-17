use serde::Deserialize;

pub use super::stream::{ContainerLogsStream, ImageCreateStream};

#[derive(Debug, Deserialize)]
pub struct ContainerInfo {
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Created")]
    pub created: u64,
    #[serde(rename = "Image")]
    pub image: String,
    #[serde(rename = "ImageID")]
    pub image_id: String,
    #[serde(rename = "Command")]
    pub command: String,
    #[serde(rename = "Status")]
    pub status: String,
}

#[derive(Debug)]
pub enum ContainerList {
    Succeeded(Vec<ContainerInfo>),
    BadParameter(ErrorResponse),
    ServerError(ErrorResponse),
}

#[derive(Debug, Deserialize)]
pub struct ContainerCreateResponse {
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Warnings")]
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum ImageCreate {
    Succeeded(ImageCreateStream),
    NoReadAccess(ErrorResponse),
    ServerError(ErrorResponse),
}

#[derive(Debug)]
pub enum ContainerCreate {
    Succeeded(ContainerCreateResponse),
    BadParameter(ErrorResponse),
    NoSuchImage(ErrorResponse),
    Conflict(ErrorResponse),
    ServerError(ErrorResponse),
}

#[derive(Debug)]
pub struct ContainerCreateSpec<'a> {
    pub image: &'a str,
    pub command: Vec<&'a str>,
}

#[derive(Debug, Deserialize)]
pub struct ContainerWaitResponseExitError {
    #[serde(rename = "Message")]
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct ContainerWaitResponse {
    #[serde(rename = "StatusCode")]
    pub status_code: i64,
    #[serde(rename = "Error")]
    pub error: Option<ContainerWaitResponseExitError>,
}

#[derive(Debug)]
pub enum ContainerWait {
    Succeeded(ContainerWaitResponse),
    BadParameter(ErrorResponse),
    NoSuchContainer(ErrorResponse),
    ServerError(ErrorResponse),
}

#[derive(Debug)]
pub enum ContainerRemove {
    Succeeded,
    BadParameter(ErrorResponse),
    NoSuchContainer(ErrorResponse),
    Conflict(ErrorResponse),
    ServerError(ErrorResponse),
}

#[derive(Debug)]
pub enum ContainerLogs {
    Succeeded(ContainerLogsStream),
    NoSuchContainer(ErrorResponse),
    ServerError(ErrorResponse),
}

#[derive(Debug)]
pub enum ContainerAttach {
    Succeeded(ContainerLogsStream),
    BadParameter(ErrorResponse),
    NoSuchContainer(ErrorResponse),
    ServerError(ErrorResponse),
}

#[derive(Debug)]
pub enum ContainerUpload {
    Succeeded,
    BadParameter(ErrorResponse),
    PermissionDenied(ErrorResponse),
    NoSuchContainer(ErrorResponse),
    ServerError(ErrorResponse),
}

#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    pub message: String,
}

#[derive(Debug)]
pub enum ContainerStart {
    Succeeded,
    AlreadyStarted,
    NoSuchContainer(ErrorResponse),
    ServerError(ErrorResponse),
}

#[derive(Debug)]
pub enum ContainerStop {
    Succeeded,
    AlreadyStopped,
    NoSuchContainer(ErrorResponse),
    ServerError(ErrorResponse),
}
