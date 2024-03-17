use super::{
    error::{TarError, TarResult},
    stream::TarStream,
};

pub enum TarEntry {
    File(String),
}

pub struct TarArchive {
    entries: Vec<TarEntry>,
}

impl TarArchive {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn append_file(&mut self, file: String) {
        self.entries.push(TarEntry::File(file));
    }

    pub fn into_stream(self, buffer_size: usize) -> TarStream {
        TarStream::new(self.entries, buffer_size)
    }
}

pub enum TarChunk {
    Header(String, Box<[u8; 512]>),
    Data(Vec<u8>),
    Padding(usize),
}

impl TarChunk {
    pub fn header(path: String, data: Box<[u8; 512]>) -> Self {
        TarChunk::Header(path, data)
    }

    pub fn padding(index: usize) -> Self {
        TarChunk::Padding(index)
    }

    pub fn data(pages: usize) -> Self {
        TarChunk::Data(vec![0; pages * 512])
    }

    pub fn len(&self) -> usize {
        match self {
            TarChunk::Header(_, data) => data.len(),
            TarChunk::Padding(_) => 512,
            TarChunk::Data(data) => data.len(),
        }
    }

    pub fn offset(&mut self, value: usize) -> TarResult<&mut [u8]> {
        match self {
            TarChunk::Padding(_) => Err(TarError::memory_access(format!(
                "Padding cannot provide offset, but requested {value}"
            ))),
            TarChunk::Header(_, data) => match data.get_mut(value..) {
                Some(data) => Ok(data),
                None => Err(TarError::memory_access(format!(
                    "Header cannot provide offset at {value}"
                ))),
            },
            TarChunk::Data(data) => {
                let length = data.len();

                match data.get_mut(value..) {
                    Some(data) => Ok(data),
                    None => Err(TarError::memory_access(format!(
                        "Data cannot provide offset at {value}, length={length}",
                    ))),
                }
            }
        }
    }
}

impl Into<Vec<u8>> for TarChunk {
    fn into(self) -> Vec<u8> {
        match self {
            TarChunk::Header(_, data) => Vec::from(*data),
            TarChunk::Padding(_) => vec![0; 512],
            TarChunk::Data(data) => data,
        }
    }
}
