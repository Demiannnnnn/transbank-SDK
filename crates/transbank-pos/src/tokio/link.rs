//! Asynchronous link layer: sends frames, waits for ACK/NAK and responses with
//! timeouts and retries (`docs/protocol-spec.md` §3, §4.2, §8).
//!
//! # Cancellation safety
//!
//! If a future returned by [`request`](AsyncLink::request) or
//! [`send_command`](AsyncLink::send_command) is dropped before completion (for
//! instance due to an external [`::tokio::time::timeout`]), the command may have
//! already been transmitted to and processed by the POS. In particular, for a sale
//! the result is **UNKNOWN**: the customer may have paid. Never resend the sale
//! blindly; call [`last_sale`](crate::tokio::AsyncPosIntegrado::last_sale) to inspect
//! the outcome before retrying.
//!
//! [`AsyncLink`] discards pending unread bytes and resets its internal decoder
//! before each new command, so subsequent operations on the same link can proceed
//! without reading stale responses from a canceled operation.
//!
//! # Runtime requirements
//!
//! All asynchronous operations on [`AsyncLink`] must run within a Tokio runtime with
//! the time driver enabled (e.g. `enable_time()`), as well as the I/O driver enabled
//! for network sockets or serial streams (e.g. `enable_all()`).

use std::collections::VecDeque;
use std::time::Duration;

use ::tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};

use crate::LinkConfig;
use crate::codes::IntermediateStatus;
use crate::error::{Error, Result};
use crate::exchange::{
    AckStep, AckWait, ListTailStep, ListTailWait, ResponseStep, ResponseWait, count_frame,
    is_list_terminator,
};
use crate::link::{TraceLayout, trace_payload};
use crate::protocol::{ACK, FrameDecoder, LinkEvent, NAK, encode_frame};

/// Size of the read buffer.
const READ_CHUNK: usize = 512;

macro_rules! link_debug {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::debug!(target: "transbank_pos::link", $($arg)*);
    };
}

/// What to do with a valid frame received while waiting for a response.
enum Received {
    Status(IntermediateStatus),
    Response(String),
}

/// Asynchronous low-level link with the POS over an [`AsyncRead`] + [`AsyncWrite`] stream.
///
/// Implements framing, ACK/NAK, retransmissions, timeouts and multi-frame lists
/// using the shared sans-IO exchange state machine without spawning background tasks.
///
/// # Runtime requirements
///
/// All operations must run within a Tokio runtime with the time driver enabled (and
/// I/O driver enabled for `TcpStream` or serial ports, e.g. `enable_all()`).
///
/// # Cancellation
///
/// Dropping an active request leaves the transaction in an **unknown** state on
/// the POS. Always query the last sale before retrying. See module-level
/// documentation for details.
///
/// ```no_run
/// # #[cfg(feature = "tokio")]
/// # async fn doc() -> Result<(), transbank_pos::Error> {
/// use transbank_pos::LinkConfig;
/// use transbank_pos::tokio::AsyncLink;
///
/// let stream = ::tokio::net::TcpStream::connect("127.0.0.1:7777").await?;
/// let mut link = AsyncLink::new(stream, LinkConfig::default());
/// link.send_command("0100").await?;
/// # Ok(())
/// # }
/// ```
pub struct AsyncLink<S: AsyncRead + AsyncWrite + Unpin + Send> {
    stream: S,
    config: LinkConfig,
    decoder: FrameDecoder,
    events: VecDeque<LinkEvent>,
    scratch: Vec<LinkEvent>,
    trace_layout: TraceLayout,
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send> AsyncLink<S> {
    /// Creates a new asynchronous link over `stream` with `config`.
    #[must_use]
    pub fn new(stream: S, config: LinkConfig) -> Self {
        Self {
            stream,
            config,
            decoder: FrameDecoder::new(),
            events: VecDeque::new(),
            scratch: Vec::new(),
            trace_layout: TraceLayout::Integrado,
        }
    }

    /// Link configuration.
    #[must_use]
    pub fn config(&self) -> &LinkConfig {
        &self.config
    }

    /// Mutable reference to the underlying stream.
    pub fn stream_mut(&mut self) -> &mut S {
        &mut self.stream
    }

    /// Consumes the link and returns the underlying stream.
    #[must_use]
    pub fn into_inner(self) -> S {
        self.stream
    }

    /// Sends a command and waits only for its ACK (`0100`, `0300`, `0260|0|`…).
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`]/[`Error::Overflow`] for an invalid payload,
    /// [`Error::NoAck`] after `max_attempts` transmissions, or an I/O error.
    pub async fn send_command(&mut self, payload: &str) -> Result<()> {
        let frame = encode_frame(payload)?;
        self.send_frame(&frame).await
    }

    /// Sends a command, waits for its ACK and for the response `expected`
    /// (function code). Intermediate `0900` messages go to `on_status`.
    ///
    /// The response payload is returned decoded as ISO-8859-1.
    ///
    /// # Errors
    ///
    /// Those of [`send_command`](Self::send_command), plus
    /// [`Error::ResponseTimeout`] (for a sale the result is **unknown**: query the
    /// last sale, never resend blindly), [`Error::BadLrc`] and
    /// [`Error::UnexpectedResponse`].
    pub async fn request(
        &mut self,
        payload: &str,
        expected: &'static str,
        on_status: &mut (dyn FnMut(IntermediateStatus) + Send),
    ) -> Result<String> {
        let frame = encode_frame(payload)?;
        self.send_frame(&frame).await?;
        let waited = self.config.response_timeout;
        let deadline = Self::deadline(waited);
        loop {
            match self.receive(expected, deadline, waited).await? {
                Received::Status(status) => on_status(status),
                Received::Response(response) => return Ok(response),
            }
        }
    }

    /// Sends a command, waits for its ACK and collects the frames `expected` until
    /// the end of the list (`docs/protocol-spec.md` §4.2).
    ///
    /// Returns the payloads of the entries with an authorization code; the
    /// terminators (empty authorization) are consumed and dropped. Each frame must
    /// arrive within `response_timeout` of the previous one.
    ///
    /// # Errors
    ///
    /// Those of [`request`](Self::request), plus [`Error::TooManyFrames`] after
    /// 10 000 frames.
    pub async fn request_list(
        &mut self,
        payload: &str,
        expected: &'static str,
    ) -> Result<Vec<String>> {
        let frame = encode_frame(payload)?;
        self.send_frame(&frame).await?;
        let mut entries = Vec::new();
        let mut frames = 0usize;
        loop {
            let waited = self.config.response_timeout;
            let deadline = Self::deadline(waited);
            let response = match self.receive(expected, deadline, waited).await? {
                Received::Status(_) => continue,
                Received::Response(response) => response,
            };
            frames = count_frame(frames)?;
            if is_list_terminator(&response) {
                break;
            }
            entries.push(response);
        }
        self.drain_list_tail(expected, &mut entries, frames).await?;
        Ok(entries)
    }

    /// Discards any unread pending bytes without blocking, and resets the frame decoder
    /// and event queue (trap b).
    async fn discard_input(&mut self) {
        self.decoder.reset();
        self.events.clear();
        let mut buf = [0u8; READ_CHUNK];
        std::future::poll_fn(|cx| {
            loop {
                let mut read_buf = ReadBuf::new(&mut buf);
                match std::pin::Pin::new(&mut self.stream).poll_read(cx, &mut read_buf) {
                    std::task::Poll::Ready(Ok(())) => {
                        if read_buf.filled().is_empty() {
                            return std::task::Poll::Ready(());
                        }
                    }
                    std::task::Poll::Ready(Err(_)) | std::task::Poll::Pending => {
                        return std::task::Poll::Ready(());
                    }
                }
            }
        })
        .await;
    }

    /// Writes `frame` until it is acknowledged (LINK-02).
    ///
    /// A valid frame received while waiting for the ACK means the POS accepted the
    /// command and the ACK was lost: it is kept for the response and treated as an
    /// implicit ACK, so the command is never written again (LINK-03).
    async fn send_frame(&mut self, frame: &[u8]) -> Result<()> {
        self.discard_input().await;
        let mut wait = AckWait::new(self.config.max_attempts);
        while wait.next_attempt() {
            if wait.attempt() > 1 {
                // Same as `Link::send_frame`: drop a partial frame from the previous attempt.
                self.decoder.reset();
            }
            let attempt = wait.attempt();
            let _ = attempt;
            self.stream.write_all(frame).await?;
            self.stream.flush().await?;
            link_debug!(len = frame.len(), attempt, "frame sent");
            let ack_deadline = Self::deadline(self.config.ack_timeout);
            loop {
                let step = match ::tokio::time::timeout_at(ack_deadline, self.next_event()).await {
                    Ok(Ok(event)) => wait.on_event(event),
                    Ok(Err(err)) => return Err(err),
                    Err(_) => {
                        link_debug!("ACK timeout");
                        AckStep::Timeout
                    }
                };
                match step {
                    AckStep::Acked => {
                        link_debug!("ACK received");
                        return Ok(());
                    }
                    AckStep::Nak => {
                        link_debug!("NAK received");
                        break;
                    }
                    AckStep::ImplicitAck(event) => {
                        link_debug!("frame received before ACK: implicit ACK");
                        self.events.push_front(event);
                        return Ok(());
                    }
                    AckStep::Timeout => break,
                    AckStep::Ignore => {}
                }
            }
        }
        Err(wait.no_ack_error())
    }

    /// Waits for the next valid frame until `deadline`: acknowledges it, NAKs bad
    /// LRCs and classifies it as intermediate status or expected response.
    async fn receive(
        &mut self,
        expected: &'static str,
        deadline: ::tokio::time::Instant,
        waited: Duration,
    ) -> Result<Received> {
        let mut wait = ResponseWait::new(self.config.max_bad_lrc, expected);
        loop {
            let event = match ::tokio::time::timeout_at(deadline, self.next_event()).await {
                Ok(Ok(event)) => event,
                Ok(Err(err)) => return Err(err),
                Err(_) => {
                    link_debug!("response timeout");
                    return Err(Error::ResponseTimeout { waited });
                }
            };
            match wait.on_event(event)? {
                ResponseStep::SendNak { count } => {
                    let _ = count;
                    link_debug!(count, "bad LRC");
                    self.stream.write_all(&[NAK]).await?;
                    self.stream.flush().await?;
                }
                ResponseStep::BadLrcExhausted { count, error } => {
                    let _ = count;
                    link_debug!(count, "bad LRC");
                    return Err(error);
                }
                ResponseStep::AckStatus { text, status } => {
                    self.stream.write_all(&[ACK]).await?;
                    self.stream.flush().await?;
                    trace_payload(&text, self.trace_layout);
                    return Ok(Received::Status(status));
                }
                ResponseStep::AckResponse { text } => {
                    self.stream.write_all(&[ACK]).await?;
                    self.stream.flush().await?;
                    trace_payload(&text, self.trace_layout);
                    return Ok(Received::Response(text));
                }
                ResponseStep::AckUnexpected { text, error } => {
                    self.stream.write_all(&[ACK]).await?;
                    self.stream.flush().await?;
                    trace_payload(&text, self.trace_layout);
                    return Err(error);
                }
                ResponseStep::Ignore => {}
            }
        }
    }

    /// After the first terminator, keeps acknowledging frames until `detail_tail`
    /// passes without any event.
    async fn drain_list_tail(
        &mut self,
        expected: &'static str,
        entries: &mut Vec<String>,
        frames: usize,
    ) -> Result<()> {
        let mut wait = ListTailWait::new(expected, frames);
        loop {
            let deadline = Self::deadline(self.config.detail_tail);
            let event = match ::tokio::time::timeout_at(deadline, self.next_event()).await {
                Ok(Ok(event)) => event,
                Ok(Err(err)) => return Err(err),
                Err(_) => return Ok(()),
            };
            match wait.on_event(event) {
                ListTailStep::SendNak => {
                    self.stream.write_all(&[NAK]).await?;
                    self.stream.flush().await?;
                }
                ListTailStep::AckEntry(text) => {
                    self.stream.write_all(&[ACK]).await?;
                    self.stream.flush().await?;
                    entries.push(text);
                }
                ListTailStep::AckTerminator => {
                    self.stream.write_all(&[ACK]).await?;
                    self.stream.flush().await?;
                }
                ListTailStep::AckError(err) => {
                    self.stream.write_all(&[ACK]).await?;
                    self.stream.flush().await?;
                    return Err(err);
                }
                ListTailStep::Ignore => {}
            }
        }
    }

    /// Next decoded event, reading from the stream.
    async fn next_event(&mut self) -> Result<LinkEvent> {
        let mut buf = [0u8; READ_CHUNK];
        loop {
            if let Some(event) = self.events.pop_front() {
                return Ok(event);
            }
            let read = self.stream.read(&mut buf).await?;
            if read == 0 {
                return Err(Error::Io(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "connection closed by the peer",
                )));
            }
            let chunk = buf.get(..read).unwrap_or_default();
            self.decoder.push(chunk, &mut self.scratch);
            self.events.extend(self.scratch.drain(..));
        }
    }

    fn deadline(timeout: Duration) -> ::tokio::time::Instant {
        const THIRTY_YEARS: Duration = Duration::from_secs(30 * 365 * 86_400);
        let bounded = timeout.min(THIRTY_YEARS);
        ::tokio::time::Instant::now() + bounded
    }
}
