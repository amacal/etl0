use std::collections::VecDeque;
use std::mem;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;

use super::core::{TarChunk, TarEntry};
use super::state::{TarPollResult, TarStateHandler};
use super::{error::TarResult, state::TarState};

pub struct TarStream {
    state: TarState,
    buffer_size: usize,
    entries: VecDeque<TarEntry>,
}

impl TarStream {
    pub fn new(entries: Vec<TarEntry>, buffer_size: usize) -> Self {
        Self {
            state: TarState::init(),
            buffer_size: buffer_size / 512 * 512,
            entries: entries.into(),
        }
    }
}

impl Stream for TarStream {
    type Item = TarResult<TarChunk>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let self_mut = self.get_mut();

        loop {
            let mut state = TarState::completed();
            mem::swap(&mut state, &mut self_mut.state);

            let result = match state {
                TarState::Init(state) => state.poll(cx),
                TarState::Open(state) => state.poll(cx),
                TarState::Header(state) => state.poll(cx),
                TarState::Read(state) => state.poll(cx),
                TarState::Padding(state) => state.poll(cx),
                TarState::Completed(state) => state.poll(cx),
            };

            let (state, poll) = match result {
                TarPollResult::ContinueLooping(state) => (state, None),
                TarPollResult::ReturnPolling(state, poll) => (state, Some(poll)),
                TarPollResult::NextEntry() => match self_mut.entries.pop_front() {
                    None => (TarState::padding(), None),
                    Some(entry) => (TarState::open(self_mut.buffer_size, entry), None),
                },
            };

            self_mut.state = state;

            if let Some(poll) = poll {
                return poll;
            }
        }
    }
}
