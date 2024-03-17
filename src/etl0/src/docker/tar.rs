use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;
use hyper::body::{Body, Bytes, Frame};

use super::error::DockerError;
use crate::tar::TarStream;

pub struct TarBody {
    inner: TarStream,
}

impl TarBody {
    pub fn from(stream: TarStream) -> Self {
        Self { inner: stream }
    }
}

impl Body for TarBody {
    type Data = Bytes;
    type Error = DockerError;

    fn poll_frame(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let self_mut: &mut TarBody = self.get_mut();
        let pointer: &mut TarStream = &mut self_mut.inner;
        let inner: Pin<&mut TarStream> = Pin::new(pointer);

        match inner.poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(chunk) => match chunk {
                None => Poll::Ready(None),
                Some(Err(error)) => Poll::Ready(Some(DockerError::raise_outgoing_archive_failed(error))),
                Some(Ok(chunk)) => {
                    let data: Vec<u8> = chunk.into();
                    let frame: Frame<Bytes> = Frame::data(Bytes::from(data));

                    Poll::Ready(Some(Ok(frame)))
                }
            },
        }
    }
}
