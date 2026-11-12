//! Sans-IO exchange logic shared by [`Link`](crate::link::Link) and
//! [`AsyncLink`](crate::tokio::AsyncLink).
//!
//! Contains the state machines for waiting for ACKs, responses and multi-frame lists
//! without any I/O, timers or background tasks.

use crate::codes::IntermediateStatus;
use crate::error::{Error, Result};
use crate::message::parse_intermediate;
use crate::protocol::{FC_INTERMEDIATE, LinkEvent, decode_latin1};

/// Maximum number of frames accepted in a multi-frame response (SAFE-02).
pub(crate) const MAX_LIST_FRAMES: usize = 10_000;

/// Index of the authorization code in a detail entry (`0261`, `0291`).
pub(crate) const DETAIL_AUTHORIZATION_FIELD: usize = 5;

/// Extracts the 4-digit function code from the payload.
pub(crate) fn function_code(payload: &str) -> &str {
    payload.split('|').next().unwrap_or_default().trim()
}

/// Builds an [`Error::UnexpectedResponse`] error.
pub(crate) fn unexpected(expected: &'static str, payload: &str) -> Error {
    Error::UnexpectedResponse {
        expected,
        received: function_code(payload).to_owned(),
        payload: payload.to_owned(),
    }
}

/// A detail entry without authorization code ends the list.
pub(crate) fn is_list_terminator(payload: &str) -> bool {
    payload
        .split('|')
        .nth(DETAIL_AUTHORIZATION_FIELD)
        .is_none_or(|field| field.trim().is_empty())
}

/// Increments frame count, returning an error if it exceeds [`MAX_LIST_FRAMES`].
pub(crate) fn count_frame(frames: usize) -> Result<usize> {
    let frames = frames.saturating_add(1);
    if frames > MAX_LIST_FRAMES {
        return Err(Error::TooManyFrames {
            max: MAX_LIST_FRAMES,
        });
    }
    Ok(frames)
}

/// Outcome of waiting for an ACK on a sent command.
#[derive(Debug)]
pub(crate) enum AckStep {
    /// Command was acknowledged (ACK received).
    Acked,
    /// NAK was received from the POS.
    Nak,
    /// An implicit ACK (Frame or `BadLrc` arrived before ACK).
    /// The event is re-enqueued for the response wait.
    ImplicitAck(LinkEvent),
    /// Wait timed out without an event.
    Timeout,
    /// Event ignored while waiting for ACK.
    Ignore,
}

/// Decision state machine for waiting for command acknowledgement.
#[derive(Debug)]
pub(crate) struct AckWait {
    max_attempts: u8,
    attempt: u8,
    naks: u8,
    implicit_ack: bool,
}

impl AckWait {
    /// Creates a new ACK waiter with a maximum number of transmission attempts.
    ///
    /// `implicit_ack`: a frame (or a frame with a bad LRC) received before the ACK
    /// counts as the ACK. Only for commands that expect a response; a command
    /// without response (`0100`, `0300`, `0070`, `0260|0|`) ends only with an ACK
    /// (`docs/protocol-spec.md` §3.3), so frames are discarded while waiting.
    pub(crate) fn new(max_attempts: u8, implicit_ack: bool) -> Self {
        Self {
            max_attempts: max_attempts.max(1),
            attempt: 0,
            naks: 0,
            implicit_ack,
        }
    }

    /// Advances to the next attempt, returning `true` if an attempt is available.
    pub(crate) fn next_attempt(&mut self) -> bool {
        if self.attempt >= self.max_attempts {
            return false;
        }
        self.attempt = self.attempt.saturating_add(1);
        true
    }

    /// Current attempt number.
    pub(crate) fn attempt(&self) -> u8 {
        self.attempt
    }

    /// Evaluates an incoming event while waiting for the ACK.
    pub(crate) fn on_event(&mut self, event: LinkEvent) -> AckStep {
        match event {
            LinkEvent::Ack => AckStep::Acked,
            LinkEvent::Nak => {
                self.naks = self.naks.saturating_add(1);
                AckStep::Nak
            }
            event @ (LinkEvent::Frame(_) | LinkEvent::BadLrc { .. }) if self.implicit_ack => {
                AckStep::ImplicitAck(event)
            }
            LinkEvent::Frame(_) | LinkEvent::BadLrc { .. } => AckStep::Ignore,
            LinkEvent::Garbage(_) | LinkEvent::Overflow => AckStep::Ignore,
        }
    }

    /// Returns [`Error::NoAck`] with the total attempts and NAKs observed.
    pub(crate) fn no_ack_error(&self) -> Error {
        Error::NoAck {
            attempts: self.max_attempts,
            naks: self.naks,
        }
    }
}

/// Outcome of processing an event while waiting for a response frame.
#[derive(Debug)]
pub(crate) enum ResponseStep {
    /// A valid intermediate status. Caller must send ACK and dispatch status.
    AckStatus {
        text: String,
        status: IntermediateStatus,
    },
    /// The expected response. Caller must send ACK and return text.
    AckResponse { text: String },
    /// Unexpected response function code. Caller must send ACK and return error.
    AckUnexpected { text: String, error: Error },
    /// Bad LRC received. Caller must send NAK.
    SendNak { count: u8 },
    /// Consecutive bad LRCs exceeded `max_bad_lrc`. Returns [`Error::BadLrc`].
    BadLrcExhausted { count: u8, error: Error },
    /// Event ignored while waiting for response.
    Ignore,
}

/// Decision state machine for waiting for a response frame.
#[derive(Debug)]
pub(crate) struct ResponseWait {
    max_bad_lrc: u8,
    bad_lrc: u8,
    expected: &'static str,
}

impl ResponseWait {
    /// Creates a new response waiter.
    pub(crate) fn new(max_bad_lrc: u8, expected: &'static str) -> Self {
        Self {
            max_bad_lrc,
            bad_lrc: 0,
            expected,
        }
    }

    /// Evaluates an incoming event while waiting for the response.
    pub(crate) fn on_event(&mut self, event: LinkEvent) -> Result<ResponseStep> {
        match event {
            LinkEvent::BadLrc { .. } => {
                self.bad_lrc = self.bad_lrc.saturating_add(1);
                if self.bad_lrc > self.max_bad_lrc {
                    Ok(ResponseStep::BadLrcExhausted {
                        count: self.bad_lrc,
                        error: Error::BadLrc {
                            count: self.bad_lrc,
                        },
                    })
                } else {
                    Ok(ResponseStep::SendNak {
                        count: self.bad_lrc,
                    })
                }
            }
            LinkEvent::Frame(payload) => {
                self.bad_lrc = 0;
                let text = decode_latin1(&payload);
                let fc = function_code(&text);
                if fc == FC_INTERMEDIATE {
                    let status = parse_intermediate(&text)?;
                    Ok(ResponseStep::AckStatus { text, status })
                } else if fc == self.expected {
                    Ok(ResponseStep::AckResponse { text })
                } else {
                    let error = unexpected(self.expected, &text);
                    Ok(ResponseStep::AckUnexpected { text, error })
                }
            }
            LinkEvent::Ack | LinkEvent::Nak | LinkEvent::Garbage(_) | LinkEvent::Overflow => {
                Ok(ResponseStep::Ignore)
            }
        }
    }
}

/// Outcome of processing an event in the list tail drain.
#[derive(Debug)]
pub(crate) enum ListTailStep {
    /// Bad LRC received in tail: caller must send NAK.
    SendNak,
    /// Another data entry received in tail: caller must send ACK and append entry.
    AckEntry(String),
    /// Terminator received in tail: caller must send ACK.
    AckTerminator,
    /// Valid frame received in tail that produced an error (e.g. frame limit or unexpected function).
    /// Caller must send ACK and return this error.
    AckError(Error),
    /// Event ignored.
    Ignore,
}

/// Decision state machine for draining the sales detail list tail.
#[derive(Debug)]
pub(crate) struct ListTailWait {
    expected: &'static str,
    frames: usize,
}

impl ListTailWait {
    /// Creates a new list tail waiter.
    pub(crate) fn new(expected: &'static str, frames: usize) -> Self {
        Self { expected, frames }
    }

    /// Evaluates an incoming event in the detail tail.
    pub(crate) fn on_event(&mut self, event: LinkEvent) -> ListTailStep {
        match event {
            LinkEvent::BadLrc { .. } => ListTailStep::SendNak,
            LinkEvent::Frame(payload) => {
                let text = decode_latin1(&payload);
                match count_frame(self.frames) {
                    Ok(next_frames) => {
                        self.frames = next_frames;
                        let fc = function_code(&text);
                        if fc != self.expected {
                            ListTailStep::AckError(unexpected(self.expected, &text))
                        } else if is_list_terminator(&text) {
                            ListTailStep::AckTerminator
                        } else {
                            ListTailStep::AckEntry(text)
                        }
                    }
                    Err(err) => {
                        self.frames = self.frames.saturating_add(1);
                        ListTailStep::AckError(err)
                    }
                }
            }
            LinkEvent::Ack | LinkEvent::Nak | LinkEvent::Garbage(_) | LinkEvent::Overflow => {
                ListTailStep::Ignore
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ack_wait_transitions() {
        let mut wait = AckWait::new(3, true);
        assert!(wait.next_attempt());
        assert_eq!(wait.attempt(), 1);

        assert!(matches!(
            wait.on_event(LinkEvent::Garbage(1)),
            AckStep::Ignore
        ));
        assert!(matches!(wait.on_event(LinkEvent::Nak), AckStep::Nak));

        assert!(wait.next_attempt());
        assert_eq!(wait.attempt(), 2);
        assert!(matches!(
            wait.on_event(LinkEvent::Frame(b"0210|00".to_vec())),
            AckStep::ImplicitAck(_)
        ));

        let mut wait3 = AckWait::new(1, true);
        assert!(wait3.next_attempt());
        assert!(!wait3.next_attempt());
        assert!(matches!(
            wait3.no_ack_error(),
            Error::NoAck {
                attempts: 1,
                naks: 0
            }
        ));
    }

    #[test]
    fn response_wait_transitions() {
        let mut wait = ResponseWait::new(1, "0210");
        assert!(matches!(
            wait.on_event(LinkEvent::Garbage(2)).unwrap(),
            ResponseStep::Ignore
        ));
        assert!(matches!(
            wait.on_event(LinkEvent::BadLrc {
                expected: 1,
                received: 2,
                payload: vec![]
            })
            .unwrap(),
            ResponseStep::SendNak { count: 1 }
        ));
        assert!(matches!(
            wait.on_event(LinkEvent::BadLrc {
                expected: 1,
                received: 2,
                payload: vec![]
            })
            .unwrap(),
            ResponseStep::BadLrcExhausted { count: 2, .. }
        ));

        let mut wait2 = ResponseWait::new(3, "0210");
        assert!(matches!(
            wait2
                .on_event(LinkEvent::Frame(b"0900|78".to_vec()))
                .unwrap(),
            ResponseStep::AckStatus { .. }
        ));
        assert!(matches!(
            wait2
                .on_event(LinkEvent::Frame(b"0210|00|ok".to_vec()))
                .unwrap(),
            ResponseStep::AckResponse { .. }
        ));
    }

    #[test]
    fn list_tail_wait_transitions() {
        let mut tail = ListTailWait::new("0261", 0);
        assert!(matches!(
            tail.on_event(LinkEvent::BadLrc {
                expected: 0,
                received: 1,
                payload: vec![]
            }),
            ListTailStep::SendNak
        ));
        assert!(matches!(
            tail.on_event(LinkEvent::Frame(
                b"0261|00|5970|IT|ABC|757752|1000".to_vec()
            )),
            ListTailStep::AckEntry(_)
        ));
        assert!(matches!(
            tail.on_event(LinkEvent::Frame(b"0261|00|5970|IT||||".to_vec())),
            ListTailStep::AckTerminator
        ));
    }

    #[test]
    fn list_tail_wait_unexpected_function_returns_ack_error() {
        let mut tail = ListTailWait::new("0261", 0);
        let step = tail.on_event(LinkEvent::Frame(b"0810|00|WRONG".to_vec()));
        assert!(matches!(
            step,
            ListTailStep::AckError(Error::UnexpectedResponse { .. })
        ));
    }

    #[test]
    fn list_tail_wait_too_many_frames_returns_ack_error() {
        let mut tail = ListTailWait::new("0261", 10_000);
        let step = tail.on_event(LinkEvent::Frame(
            b"0261|00|5970|IT|ABC|757752|1000".to_vec(),
        ));
        assert!(matches!(
            step,
            ListTailStep::AckError(Error::TooManyFrames { max: 10_000 })
        ));
    }
}
