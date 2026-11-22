//! In-memory asynchronous stream connected to a [`PosSimulator`].

use std::collections::VecDeque;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::simulator::PosSimulator;

/// In-memory asynchronous stream connected directly to a [`PosSimulator`].
///
/// Implements [`AsyncRead`] and [`AsyncWrite`] without background tasks or runtime
/// requirements.
///
/// ```
/// use transbank_pos_sim::{AsyncSimStream, PosSimulator, TerminalKind};
///
/// let sim = PosSimulator::new(TerminalKind::Integrado);
/// let stream = AsyncSimStream::new(sim);
/// ```
#[derive(Debug)]
pub struct AsyncSimStream {
    sim: PosSimulator,
    incoming: VecDeque<u8>,
    read_waker: Option<Waker>,
}

impl AsyncSimStream {
    /// Creates a new in-memory async stream wrapping `sim`.
    #[must_use]
    pub fn new(sim: PosSimulator) -> Self {
        Self {
            sim,
            incoming: VecDeque::new(),
            read_waker: None,
        }
    }

    /// Mutable reference to the underlying simulator (for example to switch scenarios).
    pub fn sim_mut(&mut self) -> &mut PosSimulator {
        &mut self.sim
    }

    /// Reference to the underlying simulator.
    #[must_use]
    pub fn sim(&self) -> &PosSimulator {
        &self.sim
    }

    /// Consumes the stream and returns the underlying simulator.
    #[must_use]
    pub fn into_inner(self) -> PosSimulator {
        self.sim
    }

    /// Injects raw bytes into the stream as if they had been received from the POS.
    pub fn push_incoming(&mut self, bytes: &[u8]) {
        self.incoming.extend(bytes);
        if let Some(waker) = self.read_waker.take() {
            waker.wake();
        }
    }
}

impl AsyncRead for AsyncSimStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.incoming.is_empty() {
            self.read_waker = Some(cx.waker().clone());
            return Poll::Pending;
        }
        let remaining = buf.remaining();
        let to_read = remaining.min(self.incoming.len());
        for _ in 0..to_read {
            if let Some(byte) = self.incoming.pop_front() {
                buf.put_slice(&[byte]);
            }
        }
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for AsyncSimStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let output = self.sim.feed(buf);
        if !output.is_empty() {
            self.incoming.extend(output);
            if let Some(waker) = self.read_waker.take() {
                waker.wake();
            }
        }
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulator::TerminalKind;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn assert_unpin_send<T: Unpin + Send>() {}

    #[test]
    fn traits_check() {
        assert_unpin_send::<AsyncSimStream>();
    }

    #[tokio::test]
    async fn read_write_and_push() {
        let sim = PosSimulator::new(TerminalKind::Integrado);
        let mut stream = AsyncSimStream::new(sim);

        // Poll command: 02 30 31 30 30 03 02
        stream
            .write_all(&[0x02, 0x30, 0x31, 0x30, 0x30, 0x03, 0x02])
            .await
            .unwrap();
        stream.flush().await.unwrap();

        let mut ack = [0u8; 1];
        stream.read_exact(&mut ack).await.unwrap();
        assert_eq!(ack[0], 0x06); // ACK

        // push_incoming
        stream.push_incoming(b"extra");
        let mut extra = [0u8; 5];
        stream.read_exact(&mut extra).await.unwrap();
        assert_eq!(&extra, b"extra");

        // sim_mut and into_inner
        let _ = stream.sim_mut();
        let _ = stream.sim();
        let _ = stream.into_inner();
    }
}
