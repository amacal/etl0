use thiserror::Error;

#[derive(Debug, Error)]
pub enum TarError {
    #[error("Cannot process file, because '{0}'")]
    IOFailed(std::io::Error),

    #[error("Cannot safely access memory, because '{0}'")]
    MemoryAccess(String),
}

impl TarError {
    pub fn memory_access(info: impl AsRef<str>) -> TarError {
        TarError::MemoryAccess(info.as_ref().to_owned())
    }
}

pub type TarResult<T> = Result<T, TarError>;
