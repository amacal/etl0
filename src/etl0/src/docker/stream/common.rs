use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use hyper::body::{Body, Bytes, Frame, Incoming};
use hyper::Response;
use tokio::task::JoinHandle;
use tokio_stream::Stream;

use crate::docker::error::{DockerError, DockerResult};
use crate::docker::http::DockerResponse;

#[derive(Debug)]
pub struct DockerStreamBuffer {
    position: usize,
    data: Vec<u8>,
}

impl DockerStreamBuffer {
    pub fn len(&self) -> usize {
        self.position
    }

    pub fn append(&mut self, data: &[u8]) {
        let expected = self.position + data.len();

        if self.data.len() < expected {
            self.data.resize(expected, 0);
        }

        let range = self.position..expected;
        let target: &mut [u8] = &mut self.data[range];

        target.copy_from_slice(data);
        self.position += data.len();
    }

    pub fn consume(&mut self, count: usize) {
        self.data.copy_within(count..self.position, 0);
        self.position -= count;
    }
}

impl AsRef<[u8]> for DockerStreamBuffer {
    fn as_ref(&self) -> &[u8] {
        &self.data[0..self.position]
    }
}

pub trait DockerStreamHandler {
    type Item;

    fn extract(&self, buffer: &mut DockerStreamBuffer) -> Vec<DockerResult<Self::Item>>;
}

#[derive(Debug)]
pub struct DockerStream<H>
where
    H: DockerStreamHandler + Sized,
    H::Item: Sized,
{
    handler: H,
    url: String,
    response: Response<Incoming>,
    connection: JoinHandle<Result<(), hyper::Error>>,
    buffer: Option<DockerStreamBuffer>,
    prefetched: VecDeque<DockerResult<H::Item>>,
}

impl<H> DockerStream<H>
where
    H: DockerStreamHandler + Sized,
    H::Item: Sized,
{
    pub fn from(handler: H, response: DockerResponse) -> Self {
        Self {
            handler: handler,
            url: response.url,
            response: response.inner,
            connection: response.connection,
            prefetched: VecDeque::new(),
            buffer: Some(DockerStreamBuffer {
                position: 0,
                data: vec![0; 65536],
            }),
        }
    }

    fn fail(&mut self, value: DockerResult<H::Item>) {
        self.prefetched.push_back(value);
        self.buffer = None;
    }

    fn append(&mut self, data: &[u8]) {
        match &mut self.buffer {
            None => (),
            Some(buffer) => buffer.append(data),
        }

        let broken = match &mut self.buffer {
            None => true,
            Some(buffer) => {
                let mut broken = false;

                for item in self.handler.extract(buffer) {
                    if let Err(_) = item {
                        broken = true;
                    }

                    self.prefetched.push_back(item);

                    if broken {
                        break;
                    }
                }

                broken
            }
        };

        if broken {
            self.buffer = None;
        }
    }
}

impl<H> DockerStream<H>
where
    H: DockerStreamHandler + Sized + Unpin,
    H::Item: Sized + Unpin,
{
    fn handle_hyper_frame(
        &mut self,
        value: Result<Frame<Bytes>, hyper::Error>,
        url: &str,
    ) -> Option<Poll<Option<<DockerStream<H> as Stream>::Item>>> {
        match value {
            Err(error) => self.fail(DockerError::raise_http_frame_failed(&url, error)),
            Ok(frame) => match frame.into_data() {
                Ok(data) => self.append(data.as_ref()),
                Err(frame) => self.fail(DockerError::raise_http_frame_unrecognized(&url, frame)),
            },
        }

        match self.prefetched.pop_front() {
            None => None,
            Some(line) => Some(Poll::Ready(Some(line))),
        }
    }

    fn handle_connection_cleanup(
        &mut self,
        cx: &mut Context<'_>,
        url: &str,
    ) -> Poll<Option<<DockerStream<H> as Stream>::Item>> {
        let pointer = &mut self.connection;
        let pin = Pin::new(pointer);

        match pin.poll(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(result) => match result {
                Ok(Err(error)) => self.fail(DockerError::raise_connection_failed(&url, error)),
                Err(error) => self.fail(DockerError::raise_tokio_failed(&url, error)),
                _ => (),
            },
        }

        match self.prefetched.pop_front() {
            None => Poll::Ready(None),
            Some(line) => Poll::Ready(Some(line)),
        }
    }
}

impl<H> Stream for DockerStream<H>
where
    H: DockerStreamHandler + Sized + Unpin,
    H::Item: Sized + Unpin,
{
    type Item = DockerResult<H::Item>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let url: String = self.url.to_owned();
        let self_mut = self.get_mut();

        loop {
            let pointer: &mut Incoming = self_mut.response.body_mut();
            let pin: Pin<&mut Incoming> = Pin::new(pointer);

            let result = match pin.poll_frame(cx) {
                Poll::Ready(value) => match value {
                    // if no more incoming data we need to flush
                    // prefetched lines and clean up the connection
                    None => match self_mut.prefetched.pop_front() {
                        None => Some(self_mut.handle_connection_cleanup(cx, &url)),
                        Some(line) => Some(Poll::Ready(Some(line))),
                    },
                    Some(value) => {
                        // either we have something to return
                        // or we need to trigger polling again
                        match self_mut.handle_hyper_frame(value, &url) {
                            None => None,
                            Some(value) => Some(value),
                        }
                    }
                },
                Poll::Pending => Some(Poll::Pending),
            };

            // none results forces additional loop iterations
            if let Some(value) = result {
                return value;
            }
        }
    }
}
