use std::fs::Metadata;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Future;
use tokio::fs::File;
use tokio::io::AsyncRead;
use tokio::io::ReadBuf;

use super::core::{TarChunk, TarEntry};
use super::error::{TarError, TarResult};
use super::header::TarHeader;

pub trait TarStateHandler {
    fn poll(self, cx: &mut Context<'_>) -> TarPollResult;
}

pub struct TarStateInit {}

impl TarStateInit {
    fn new() -> Self {
        Self {}
    }
}

impl TarStateHandler for TarStateInit {
    fn poll(self, _cx: &mut Context<'_>) -> TarPollResult {
        TarPollResult::NextEntry()
    }
}

pub struct TarStateOpen {
    buffer_size: usize,
    task: Pin<Box<dyn Future<Output = Result<(String, File), std::io::Error>> + Send>>,
}

impl TarStateOpen {
    fn new(buffer_size: usize, entry: TarEntry) -> Self {
        let task = async move {
            match entry {
                TarEntry::File(path) => match File::open(&path).await {
                    Ok(file) => Ok((path, file)),
                    Err(error) => Err(error),
                },
            }
        };

        Self {
            buffer_size: buffer_size,
            task: Box::pin(task),
        }
    }
}

impl TarStateHandler for TarStateOpen {
    fn poll(mut self, cx: &mut Context<'_>) -> TarPollResult {
        let (path, file) = match self.task.as_mut().poll(cx) {
            Poll::Pending => return TarState::Open(self).pending(),
            Poll::Ready(Err(error)) => return TarState::failed(TarError::IOFailed(error)),
            Poll::Ready(Ok((path, file))) => (path, file),
        };

        TarStateHeader::new(self.buffer_size, path, file).poll(cx)
    }
}

pub struct TarStateHeader {
    buffer_size: usize,
    path: String,
    task: Pin<Box<dyn Future<Output = Result<(File, Metadata), std::io::Error>> + Send>>,
}

impl TarStateHeader {
    fn new<'a>(buffer_size: usize, path: String, file: File) -> TarStateHeader {
        let task = async move {
            match file.metadata().await {
                Ok(metadata) => Ok((file, metadata)),
                Err(error) => Err(error),
            }
        };

        Self {
            path: path,
            task: Box::pin(task),
            buffer_size: buffer_size,
        }
    }
}

impl TarStateHandler for TarStateHeader {
    fn poll(mut self, cx: &mut Context<'_>) -> TarPollResult {
        let (file, metadata) = match self.task.as_mut().poll(cx) {
            Poll::Pending => return TarState::Header(self).pending(),
            Poll::Ready(Err(error)) => return TarState::failed(TarError::IOFailed(error)),
            Poll::Ready(Ok(metadata)) => metadata,
        };

        let length: u64 = metadata.len();
        let header: TarHeader = TarHeader::empty(self.path);

        match header.write(&metadata) {
            Ok(chunk) => TarState::read(self.buffer_size, file, length).ready(chunk),
            Err(error) => TarState::failed(error),
        }
    }
}

pub struct TarStateRead {
    buffer_size: usize,
    file: File,
    left: usize,
    completed: usize,
    chunk: TarChunk,
    offset: usize,
}

impl TarStateRead {
    fn new(buffer_size: usize, file: File, length: u64) -> Self {
        let left = length as usize / 512;
        let available = buffer_size / 512;

        let pages = std::cmp::min(available, left);
        let pages = pages + if length as usize > 0 { 1 } else { 0 };

        Self {
            buffer_size: buffer_size,
            file: file,
            left: length as usize,
            completed: 0,
            chunk: TarChunk::data(pages),
            offset: 0,
        }
    }

    fn advance(self, bytes: usize) -> Self {
        Self {
            buffer_size: self.buffer_size,
            file: self.file,
            left: self.left - bytes,
            completed: self.completed + bytes,
            chunk: self.chunk,
            offset: self.offset + bytes,
        }
    }

    fn next(self) -> (TarChunk, Self) {
        let left = self.left / 512;
        let available = self.buffer_size / 512;

        let pages = std::cmp::min(available, left);
        let pages = pages + if self.left % 512 > 0 { 1 } else { 0 };

        (
            self.chunk,
            Self {
                buffer_size: self.buffer_size,
                file: self.file,
                left: self.left,
                completed: self.completed,
                chunk: TarChunk::data(pages),
                offset: 0,
            },
        )
    }
}

impl TarStateHandler for TarStateRead {
    fn poll(mut self, cx: &mut Context<'_>) -> TarPollResult {
        let pinned: Pin<&mut File> = Pin::new(&mut self.file);
        let data = match self.chunk.offset(self.offset) {
            Err(error) => return TarState::failed(error),
            Ok(data) => data,
        };

        let mut buffer: ReadBuf<'_> = ReadBuf::new(data);
        match pinned.poll_read(cx, &mut buffer) {
            Poll::Pending => return TarState::Read(self).pending(),
            Poll::Ready(Err(error)) => return TarState::failed(TarError::IOFailed(error)),
            _ => (),
        }

        let read: usize = buffer.filled().len();
        let advanced: TarStateRead = self.advance(read);

        if advanced.left == 0 {
            return TarState::init().ready(advanced.chunk);
        }

        if advanced.offset == advanced.chunk.len() {
            let (chunk, state) = advanced.next();
            return TarState::from(TarState::Read(state)).ready(chunk);
        }

        TarState::from(TarState::Read(advanced)).looping()
    }
}

pub struct TarStatePadding {
    index: usize,
}

impl TarStatePadding {
    fn new() -> Self {
        Self { index: 0 }
    }

    fn next(self) -> Self {
        Self { index: self.index + 1 }
    }
}

impl TarStateHandler for TarStatePadding {
    fn poll(self, _cx: &mut Context<'_>) -> TarPollResult {
        match self.index {
            index if index <= 1 => TarPollResult::ReturnPolling(
                TarState::Padding(self.next()),
                Poll::Ready(Some(Ok(TarChunk::padding(index)))),
            ),
            _ => TarPollResult::ReturnPolling(TarState::completed(), Poll::Ready(None)),
        }
    }
}

pub struct TarStateCompleted {}

impl TarStateCompleted {
    fn new() -> Self {
        Self {}
    }
}

impl TarStateHandler for TarStateCompleted {
    fn poll(self, _cx: &mut Context<'_>) -> TarPollResult {
        TarPollResult::ReturnPolling(TarState::Completed(self), Poll::Ready(None))
    }
}

pub enum TarState {
    Init(TarStateInit),
    Open(TarStateOpen),
    Header(TarStateHeader),
    Read(TarStateRead),
    Padding(TarStatePadding),
    Completed(TarStateCompleted),
}

impl TarState {
    pub fn init() -> Self {
        TarState::Init(TarStateInit::new())
    }

    pub fn completed() -> Self {
        TarState::Completed(TarStateCompleted::new())
    }

    pub fn padding() -> Self {
        TarState::Padding(TarStatePadding::new())
    }

    pub fn open(buffer_size: usize, entry: TarEntry) -> Self {
        TarState::Open(TarStateOpen::new(buffer_size, entry))
    }

    pub fn read(buffer_size: usize, file: File, length: u64) -> Self {
        TarState::Read(TarStateRead::new(buffer_size, file, length))
    }

    fn pending(self) -> TarPollResult {
        TarPollResult::ReturnPolling(self, Poll::Pending)
    }

    fn ready(self, chunk: TarChunk) -> TarPollResult {
        TarPollResult::ReturnPolling(self, Poll::Ready(Some(Ok(chunk))))
    }

    fn looping(self) -> TarPollResult {
        TarPollResult::ContinueLooping(self)
    }

    fn failed(error: TarError) -> TarPollResult {
        TarPollResult::ReturnPolling(Self::completed(), Poll::Ready(Some(Err(error))))
    }
}

pub enum TarPollResult {
    NextEntry(),
    ReturnPolling(TarState, Poll<Option<TarResult<TarChunk>>>),
    ContinueLooping(TarState),
}
