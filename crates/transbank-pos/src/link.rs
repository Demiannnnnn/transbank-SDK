//! Link layer: sends frames, waits for ACK/NAK and responses with timeouts and retries
//! (`docs/protocol-spec.md` §3, §4.2, §8).
//!
//! This is the only place with time and retries. A frame is retransmitted only while
//! it has not been acknowledged; once the POS acknowledged a command, the link never
//! writes it again (LINK-03, ADR-04).

use std::collections::VecDeque;
use std::time::Duration;

use crate::codes::IntermediateStatus;
use crate::error::{Error, Result};
use crate::message::parse_intermediate;
use crate::protocol::{
    ACK, FC_INTERMEDIATE, FrameDecoder, LinkEvent, NAK, decode_latin1, encode_frame,
};
use crate::transport::{Clock, SystemClock, Transport};

/// Maximum number of frames accepted in a multi-frame response (SAFE-02).
const MAX_LIST_FRAMES: usize = 10_000;

/// Size of the read buffer.
const READ_CHUNK: usize = 512;

/// Index of the authorization code in a detail entry (`0261`, `0291`).
const DETAIL_AUTHORIZATION_FIELD: usize = 5;

macro_rules! link_debug {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::debug!(target: "transbank_pos::link", $($arg)*);
    };
}

/// Timeouts and retry limits of the link (LINK-04).
///
/// ```
/// use std::time::Duration;
/// use transbank_pos::LinkConfig;
///
/// let mut config = LinkConfig::default();
/// assert_eq!(config.ack_timeout, Duration::from_secs(2));
/// config.response_timeout = Duration::from_secs(60);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct LinkConfig {
    /// Wait for the ACK of a sent frame (default 2 s).
    pub ack_timeout: Duration,
    /// Total wait for the response after the ACK (default 150 s). Intermediate
    /// `0900` messages do not extend it.
    pub response_timeout: Duration,
    /// Silence that ends a sales detail list after its first terminator (default 500 ms).
    pub detail_tail: Duration,
    /// Transmissions of a frame without ACK before failing (default 3).
    pub max_attempts: u8,
    /// Consecutive frames with a bad LRC answered with NAK before failing (default 3).
    pub max_bad_lrc: u8,
}

impl Default for LinkConfig {
    fn default() -> Self {
        Self {
            ack_timeout: Duration::from_secs(2),
            response_timeout: Duration::from_secs(150),
            detail_tail: Duration::from_millis(500),
            max_attempts: 3,
            max_bad_lrc: 3,
        }
    }
}

/// What to do with a valid frame received while waiting for a response.
enum Received {
    /// A `0900` intermediate message (already acknowledged).
    Status(IntermediateStatus),
    /// The expected response (already acknowledged).
    Response(String),
}

/// Low-level link with the POS: framing, ACK/NAK, retries and timeouts.
///
/// The clients (`PosIntegrado`, `PosAutoservicio`) use it internally; it is public
/// for advanced users. Every method takes `&mut self`: one operation at a time
/// (LINK-05).
///
/// ```
/// use transbank_pos::{Link, LinkConfig};
/// use transbank_pos::protocol::encode_frame;
/// use transbank_pos::transport::{ManualClock, MemoryTransport};
///
/// let clock = ManualClock::new();
/// let response = encode_frame("0710|00|002|15000||")?;
/// let transport = MemoryTransport::new(clock.clone())
///     .expect_write(&encode_frame("0700||")?)
///     .then_reply(&[0x06])
///     .then_reply(&response)
///     .expect_write(&[0x06]);
/// let mut link = Link::with_clock(transport, LinkConfig::default(), clock);
/// let payload = link.request("0700||", "0710", &mut |_| {})?;
/// assert_eq!(payload, "0710|00|002|15000||");
/// # Ok::<(), transbank_pos::Error>(())
/// ```
pub struct Link<T: Transport, C: Clock = SystemClock> {
    transport: T,
    config: LinkConfig,
    clock: C,
    decoder: FrameDecoder,
    events: VecDeque<LinkEvent>,
    scratch: Vec<LinkEvent>,
}

impl<T: Transport> Link<T> {
    /// Creates a link with the system clock.
    #[must_use]
    pub fn new(transport: T, config: LinkConfig) -> Self {
        Self::with_clock(transport, config, SystemClock)
    }
}

impl<T: Transport, C: Clock> Link<T, C> {
    /// Creates a link with an explicit clock (tests use a
    /// [`ManualClock`](crate::transport::ManualClock)).
    #[must_use]
    pub fn with_clock(transport: T, config: LinkConfig, clock: C) -> Self {
        Self {
            transport,
            config,
            clock,
            decoder: FrameDecoder::new(),
            events: VecDeque::new(),
            scratch: Vec::new(),
        }
    }

    /// Sends a command and waits only for its ACK (`0100`, `0300`, `0260|0|`…).
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`]/[`Error::Overflow`] for an invalid payload,
    /// [`Error::NoAck`] after `max_attempts` transmissions, or a transport error.
    pub fn send_command(&mut self, payload: &str) -> Result<()> {
        let frame = encode_frame(payload)?;
        self.send_frame(&frame)
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
    pub fn request(
        &mut self,
        payload: &str,
        expected: &'static str,
        on_status: &mut dyn FnMut(IntermediateStatus),
    ) -> Result<String> {
        let frame = encode_frame(payload)?;
        self.send_frame(&frame)?;
        let deadline = self.deadline(self.config.response_timeout);
        loop {
            match self.receive(expected, deadline, self.config.response_timeout)? {
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
    pub fn request_list(&mut self, payload: &str, expected: &'static str) -> Result<Vec<String>> {
        let frame = encode_frame(payload)?;
        self.send_frame(&frame)?;
        let mut entries = Vec::new();
        let mut frames = 0usize;
        loop {
            let deadline = self.deadline(self.config.response_timeout);
            let response = match self.receive(expected, deadline, self.config.response_timeout)? {
                Received::Status(_) => continue,
                Received::Response(response) => response,
            };
            frames = count_frame(frames)?;
            if is_list_terminator(&response) {
                break;
            }
            entries.push(response);
        }
        self.drain_list_tail(expected, &mut entries, frames)?;
        Ok(entries)
    }

    /// Link configuration.
    #[must_use]
    pub fn config(&self) -> &LinkConfig {
        &self.config
    }

    /// The transport (for example to inspect a test transport).
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    /// Returns the transport.
    #[must_use]
    pub fn into_inner(self) -> T {
        self.transport
    }

    /// After the first terminator, keeps acknowledging frames until `detail_tail`
    /// passes without any event.
    fn drain_list_tail(
        &mut self,
        expected: &'static str,
        entries: &mut Vec<String>,
        mut frames: usize,
    ) -> Result<()> {
        loop {
            let deadline = self.deadline(self.config.detail_tail);
            let Some(event) = self.next_event(deadline)? else {
                return Ok(());
            };
            let LinkEvent::Frame(payload) = event else {
                // NAK on a bad LRC so the POS retransmits; anything else is ignored.
                if matches!(event, LinkEvent::BadLrc { .. }) {
                    self.transport.write_all(&[NAK])?;
                }
                continue;
            };
            self.transport.write_all(&[ACK])?;
            frames = count_frame(frames)?;
            let text = decode_latin1(&payload);
            let function_code = function_code(&text);
            if function_code != expected {
                return Err(unexpected(expected, &text));
            }
            if !is_list_terminator(&text) {
                entries.push(text);
            }
        }
    }

    /// Writes `frame` until it is acknowledged (LINK-02).
    ///
    /// A valid frame received while waiting for the ACK means the POS accepted the
    /// command and the ACK was lost: it is kept for the response and treated as an
    /// implicit ACK, so the command is never written again (LINK-03).
    fn send_frame(&mut self, frame: &[u8]) -> Result<()> {
        self.transport.discard_input()?;
        self.decoder.reset();
        self.events.clear();
        let attempts = self.config.max_attempts.max(1);
        let mut naks = 0u8;
        let mut attempt = 0u8;
        while attempt < attempts {
            attempt += 1;
            self.transport.write_all(frame)?;
            link_debug!(len = frame.len(), attempt, "frame sent");
            let deadline = self.deadline(self.config.ack_timeout);
            loop {
                match self.next_event(deadline)? {
                    Some(LinkEvent::Ack) => {
                        link_debug!("ACK received");
                        return Ok(());
                    }
                    Some(LinkEvent::Nak) => {
                        link_debug!("NAK received");
                        naks = naks.saturating_add(1);
                        break;
                    }
                    Some(event @ (LinkEvent::Frame(_) | LinkEvent::BadLrc { .. })) => {
                        link_debug!("frame received before ACK: implicit ACK");
                        self.events.push_front(event);
                        return Ok(());
                    }
                    Some(_) => {}
                    None => {
                        link_debug!("ACK timeout");
                        break;
                    }
                }
            }
        }
        Err(Error::NoAck { attempts, naks })
    }

    /// Waits for the next valid frame until `deadline`: acknowledges it, NAKs bad
    /// LRCs and classifies it as intermediate status or expected response.
    fn receive(
        &mut self,
        expected: &'static str,
        deadline: Duration,
        waited: Duration,
    ) -> Result<Received> {
        let mut bad_lrc = 0u8;
        loop {
            match self.next_event(deadline)? {
                None => {
                    link_debug!("response timeout");
                    return Err(Error::ResponseTimeout { waited });
                }
                Some(LinkEvent::BadLrc { .. }) => {
                    bad_lrc = bad_lrc.saturating_add(1);
                    link_debug!(count = bad_lrc, "bad LRC");
                    if bad_lrc > self.config.max_bad_lrc {
                        return Err(Error::BadLrc { count: bad_lrc });
                    }
                    self.transport.write_all(&[NAK])?;
                }
                Some(LinkEvent::Frame(payload)) => {
                    self.transport.write_all(&[ACK])?;
                    let text = decode_latin1(&payload);
                    trace_payload(&text);
                    let function_code = function_code(&text);
                    if function_code == FC_INTERMEDIATE {
                        return parse_intermediate(&text).map(Received::Status);
                    }
                    if function_code == expected {
                        return Ok(Received::Response(text));
                    }
                    return Err(unexpected(expected, &text));
                }
                Some(_) => {}
            }
        }
    }

    /// Next decoded event, reading from the transport until `deadline`.
    fn next_event(&mut self, deadline: Duration) -> Result<Option<LinkEvent>> {
        let mut buf = [0u8; READ_CHUNK];
        loop {
            if let Some(event) = self.events.pop_front() {
                return Ok(Some(event));
            }
            let remaining = deadline.saturating_sub(self.clock.now());
            if remaining.is_zero() {
                return Ok(None);
            }
            let read = self.transport.read_timeout(&mut buf, remaining)?;
            let chunk = buf.get(..read).unwrap_or_default();
            self.decoder.push(chunk, &mut self.scratch);
            self.events.extend(self.scratch.drain(..));
        }
    }

    fn deadline(&self, timeout: Duration) -> Duration {
        self.clock.now().saturating_add(timeout)
    }
}

fn count_frame(frames: usize) -> Result<usize> {
    let frames = frames.saturating_add(1);
    if frames > MAX_LIST_FRAMES {
        return Err(Error::TooManyFrames {
            max: MAX_LIST_FRAMES,
        });
    }
    Ok(frames)
}

fn function_code(payload: &str) -> &str {
    payload.split('|').next().unwrap_or_default().trim()
}

fn unexpected(expected: &'static str, payload: &str) -> Error {
    Error::UnexpectedResponse {
        expected,
        received: function_code(payload).to_owned(),
        payload: payload.to_owned(),
    }
}

/// A detail entry without authorization code ends the list.
fn is_list_terminator(payload: &str) -> bool {
    payload
        .split('|')
        .nth(DETAIL_AUTHORIZATION_FIELD)
        .is_none_or(|field| field.trim().is_empty())
}

#[cfg(feature = "tracing")]
fn trace_payload(payload: &str) {
    tracing::trace!(target: "transbank_pos::link", payload = %redact(payload), "frame received");
}

#[cfg(not(feature = "tracing"))]
fn trace_payload(_payload: &str) {}

/// Replaces the account number and voucher fields by `<redacted N>` according to the
/// layout of the function code (`security-model.md` §3).
#[cfg(feature = "tracing")]
fn redact(payload: &str) -> String {
    let sensitive: &[usize] = match function_code(payload) {
        "0210" | "0260" | "0271" | "0281" => &[13, 19],
        "0291" => &[13],
        "0261" => &[11],
        "0510" => &[4],
        "0710" | "0810" | "1210" | "0900" => &[],
        other => return format!("<fc={other}, len={}>", payload.len()),
    };
    payload
        .split('|')
        .enumerate()
        .map(|(index, field)| {
            if sensitive.contains(&index) && !field.is_empty() {
                format!("<redacted {}>", field.chars().count())
            } else {
                field.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("|")
}

#[cfg(all(test, feature = "tracing"))]
mod redact_tests {
    use super::redact;

    #[test]
    fn redacts_by_layout() {
        assert_eq!(
            redact(
                "0210|00|1|T|t|a|1|0|0|1234|1|CR|01012025|ACCOUNT|MC|01012025|101010|1|0|VOUCHER"
            ),
            "0210|00|1|T|t|a|1|0|0|1234|1|CR|01012025|<redacted 7>|MC|01012025|101010|1|0|<redacted 7>"
        );
        assert_eq!(redact("9999|secret"), "<fc=9999, len=11>");
        assert_eq!(redact("0710|00|2|100"), "0710|00|2|100");
    }
}
