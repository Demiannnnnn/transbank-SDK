//! Frame decoder: state machine of `docs/protocol-spec.md` §2.4 (ADR-05).

use std::mem;

use super::{ACK, ETX, MAX_PAYLOAD, NAK, STX, lrc};

/// Event recognised by [`FrameDecoder`] in the incoming byte stream.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LinkEvent {
    /// A single `ACK` byte outside a frame.
    Ack,
    /// A single `NAK` byte outside a frame.
    Nak,
    /// A complete frame with a valid LRC; holds the payload without `STX`, `ETX` and LRC.
    Frame(Vec<u8>),
    /// A complete frame whose LRC does not match its payload.
    BadLrc {
        /// Payload of the frame, without `STX`, `ETX` and LRC.
        payload: Vec<u8>,
        /// LRC computed from the payload.
        expected: u8,
        /// LRC byte received.
        received: u8,
    },
    /// Number of bytes discarded outside a frame, or of an incomplete frame
    /// interrupted by a new `STX`.
    Garbage(usize),
    /// A frame exceeded [`MAX_PAYLOAD`](super::MAX_PAYLOAD) bytes and was discarded.
    Overflow,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum State {
    #[default]
    Idle,
    InFrame,
    AwaitLrc,
}

/// Sans-IO decoder of frames, `ACK` and `NAK` from chunks of any size.
///
/// The byte after `ETX` is always read as the LRC without being interpreted, so an
/// LRC equal to `STX`, `ETX`, `ACK` or `NAK` is handled correctly (PROTO-03).
///
/// Bytes discarded while idle are grouped and reported as a single
/// [`LinkEvent::Garbage`] right before the next `ACK`, `NAK` or `STX`. Therefore the
/// events do not depend on how the stream is split into chunks; trailing garbage
/// stays pending until a significant byte arrives (or [`reset`](Self::reset)).
///
/// ```
/// use transbank_pos::protocol::{FrameDecoder, LinkEvent};
///
/// let mut decoder = FrameDecoder::new();
/// let mut events = Vec::new();
/// // ACK followed by the frame "0100", whose LRC (0x02) equals STX, split in two chunks.
/// decoder.push(&[0x06, 0x02, b'0', b'1'], &mut events);
/// decoder.push(&[b'0', b'0', 0x03, 0x02], &mut events);
/// assert_eq!(events, [LinkEvent::Ack, LinkEvent::Frame(b"0100".to_vec())]);
/// assert!(decoder.is_idle());
/// ```
#[derive(Debug, Default)]
pub struct FrameDecoder {
    state: State,
    payload: Vec<u8>,
    garbage: usize,
}

impl FrameDecoder {
    /// Creates an idle decoder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Processes a chunk of bytes and appends the recognised events to `out`.
    ///
    /// Nothing is allocated in `out` when the chunk produces no events.
    pub fn push(&mut self, bytes: &[u8], out: &mut Vec<LinkEvent>) {
        for &byte in bytes {
            match self.state {
                State::Idle => self.idle(byte, out),
                State::InFrame => self.in_frame(byte, out),
                State::AwaitLrc => self.await_lrc(byte, out),
            }
        }
    }

    /// Returns `true` when no frame is partially received.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.state == State::Idle
    }

    /// Discards any partial frame and pending garbage.
    pub fn reset(&mut self) {
        self.state = State::Idle;
        self.payload.clear();
        self.garbage = 0;
    }

    fn idle(&mut self, byte: u8, out: &mut Vec<LinkEvent>) {
        match byte {
            ACK => {
                self.flush_garbage(out);
                out.push(LinkEvent::Ack);
            }
            NAK => {
                self.flush_garbage(out);
                out.push(LinkEvent::Nak);
            }
            STX => {
                self.flush_garbage(out);
                self.payload.clear();
                self.state = State::InFrame;
            }
            _ => self.garbage = self.garbage.saturating_add(1),
        }
    }

    fn in_frame(&mut self, byte: u8, out: &mut Vec<LinkEvent>) {
        match byte {
            ETX => self.state = State::AwaitLrc,
            STX => {
                // The previous frame was incomplete: its STX and payload are discarded.
                out.push(LinkEvent::Garbage(self.payload.len().saturating_add(1)));
                self.payload.clear();
            }
            _ if self.payload.len() >= MAX_PAYLOAD => {
                out.push(LinkEvent::Overflow);
                self.payload.clear();
                self.state = State::Idle;
            }
            _ => self.payload.push(byte),
        }
    }

    fn await_lrc(&mut self, received: u8, out: &mut Vec<LinkEvent>) {
        let payload = mem::take(&mut self.payload);
        let expected = lrc(&payload);
        self.state = State::Idle;
        if received == expected {
            out.push(LinkEvent::Frame(payload));
        } else {
            out.push(LinkEvent::BadLrc {
                payload,
                expected,
                received,
            });
        }
    }

    fn flush_garbage(&mut self, out: &mut Vec<LinkEvent>) {
        if self.garbage > 0 {
            out.push(LinkEvent::Garbage(mem::take(&mut self.garbage)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::encode_frame;

    fn decode(bytes: &[u8]) -> Vec<LinkEvent> {
        let mut decoder = FrameDecoder::new();
        let mut events = Vec::new();
        decoder.push(bytes, &mut events);
        events
    }

    fn frame(payload: &str) -> Vec<u8> {
        encode_frame(payload).unwrap()
    }

    #[test]
    fn decodes_spec_vectors() {
        for payload in [
            "0100",
            "0300",
            "0800",
            "0500||",
            "0700||",
            "0250|",
            "0260|1|",
            "1200|000142|",
            "0200|000012000|ABC123||0|0",
        ] {
            assert_eq!(
                decode(&frame(payload)),
                [LinkEvent::Frame(payload.as_bytes().to_vec())],
                "{payload}"
            );
        }
    }

    #[test]
    fn lrc_equal_to_control_bytes_in_one_chunk() {
        let mut bytes = frame("0100");
        bytes.extend(frame("0500||"));
        assert_eq!(
            decode(&bytes),
            [
                LinkEvent::Frame(b"0100".to_vec()),
                LinkEvent::Frame(b"0500||".to_vec()),
            ]
        );
    }

    #[test]
    fn bad_lrc_reports_both_values() {
        let mut bytes = frame("0800");
        *bytes.last_mut().unwrap() = 0x0C;
        assert_eq!(
            decode(&bytes),
            [LinkEvent::BadLrc {
                payload: b"0800".to_vec(),
                expected: 0x0B,
                received: 0x0C,
            }]
        );
    }

    #[test]
    fn loose_ack_and_nak() {
        assert_eq!(
            decode(&[ACK, NAK, ACK]),
            [LinkEvent::Ack, LinkEvent::Nak, LinkEvent::Ack]
        );
    }

    #[test]
    fn garbage_before_frame_is_grouped() {
        let mut bytes = b"\xFFxyz".to_vec();
        bytes.extend(frame("0100"));
        assert_eq!(
            decode(&bytes),
            [LinkEvent::Garbage(4), LinkEvent::Frame(b"0100".to_vec())]
        );
    }

    #[test]
    fn trailing_garbage_waits_for_a_significant_byte() {
        let mut decoder = FrameDecoder::new();
        let mut events = Vec::new();
        decoder.push(b"zz", &mut events);
        assert_eq!(events, []);
        assert_eq!(events.capacity(), 0, "no allocation without events");
        decoder.push(&[ACK], &mut events);
        assert_eq!(events, [LinkEvent::Garbage(2), LinkEvent::Ack]);
    }

    #[test]
    fn stx_inside_frame_restarts() {
        let mut bytes = vec![STX, b'0', b'2'];
        bytes.extend(frame("0300"));
        assert_eq!(
            decode(&bytes),
            [LinkEvent::Garbage(3), LinkEvent::Frame(b"0300".to_vec())]
        );
    }

    #[test]
    fn overflow_then_recovers() {
        let mut bytes = vec![STX];
        bytes.extend(std::iter::repeat_n(b'1', MAX_PAYLOAD + 1));
        bytes.push(ETX);
        bytes.push(0x00);
        bytes.extend(frame("0100"));
        assert_eq!(
            decode(&bytes),
            [
                LinkEvent::Overflow,
                LinkEvent::Garbage(2),
                LinkEvent::Frame(b"0100".to_vec()),
            ]
        );
    }

    #[test]
    fn max_payload_is_accepted() {
        let payload = "1".repeat(MAX_PAYLOAD);
        assert_eq!(
            decode(&frame(&payload)),
            [LinkEvent::Frame(payload.into_bytes())]
        );
    }

    #[test]
    fn idle_and_reset() {
        let mut decoder = FrameDecoder::new();
        let mut events = Vec::new();
        assert!(decoder.is_idle());
        decoder.push(&[STX, b'0'], &mut events);
        assert!(!decoder.is_idle());
        decoder.push(&[ETX], &mut events);
        assert!(!decoder.is_idle());
        decoder.reset();
        assert!(decoder.is_idle());
        decoder.push(&frame("0100"), &mut events);
        assert_eq!(events, [LinkEvent::Frame(b"0100".to_vec())]);
    }
}
