//! In-memory scripted transport and manual clock for deterministic tests (QA-03).

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use super::{Clock, Transport};
use crate::error::{Error, Result};

/// Clock that only moves when told to. Clones share the same time.
///
/// ```
/// use std::time::Duration;
/// use transbank_pos::Clock;
/// use transbank_pos::transport::ManualClock;
///
/// let clock = ManualClock::new();
/// let shared = clock.clone();
/// clock.advance(Duration::from_secs(2));
/// assert_eq!(shared.now(), Duration::from_secs(2));
/// ```
#[derive(Debug, Clone, Default)]
pub struct ManualClock {
    now: Arc<Mutex<Duration>>,
}

impl ManualClock {
    /// Creates a clock at time zero.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn time(&self) -> MutexGuard<'_, Duration> {
        // The guarded value is a plain `Duration`: a poisoned lock still holds a valid time.
        self.now.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Moves the clock forward (saturating at [`Duration::MAX`]).
    pub fn advance(&self, by: Duration) {
        let mut now = self.time();
        *now = now.saturating_add(by);
    }

    /// Current time.
    #[must_use]
    pub fn now(&self) -> Duration {
        *self.time()
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Duration {
        ManualClock::now(self)
    }
}

#[derive(Debug)]
enum Step {
    Expect(Vec<u8>),
    Reply { delay: Duration, bytes: Vec<u8> },
}

/// Scripted in-memory transport that plays the POS side in tests.
///
/// The script is a sequence of expected writes ([`expect_write`](Self::expect_write))
/// and replies ([`then_reply`](Self::then_reply),
/// [`then_reply_after`](Self::then_reply_after)). A reply becomes readable once every
/// expected write before it has happened (and its delay has elapsed). A write that
/// does not match the next expected write, or that comes after the script ended,
/// fails with [`Error::Io`] (`InvalidData`) showing both values in hex.
///
/// [`read_timeout`](Transport::read_timeout) without readable data **advances the
/// [`ManualClock`]** by the timeout (or until the next delayed reply) instead of
/// sleeping, so timeouts are tested in microseconds of real time.
///
/// ```
/// use std::time::Duration;
/// use transbank_pos::{Link, LinkConfig};
/// use transbank_pos::transport::{ManualClock, MemoryTransport};
///
/// let clock = ManualClock::new();
/// let transport = MemoryTransport::new(clock.clone())
///     .expect_write(&[0x02, b'0', b'1', b'0', b'0', 0x03, 0x02]) // 0100
///     .then_reply(&[0x06]);                                      // ACK
/// let mut link = Link::with_clock(transport, LinkConfig::default(), clock.clone());
/// link.send_command("0100")?;
/// assert!(link.transport_mut().is_finished());
/// assert_eq!(clock.now(), Duration::ZERO);
/// # Ok::<(), transbank_pos::Error>(())
/// ```
#[derive(Debug)]
pub struct MemoryTransport {
    clock: ManualClock,
    script: VecDeque<Step>,
    pending: VecDeque<(Duration, Vec<u8>)>,
    last_due: Duration,
    incoming: VecDeque<u8>,
    written: Vec<u8>,
}

impl MemoryTransport {
    /// Creates an empty script driven by `clock`.
    #[must_use]
    pub fn new(clock: ManualClock) -> Self {
        Self {
            clock,
            script: VecDeque::new(),
            pending: VecDeque::new(),
            last_due: Duration::ZERO,
            incoming: VecDeque::new(),
            written: Vec::new(),
        }
    }

    /// Appends an expected write (compared with each `write_all` call).
    #[must_use]
    pub fn expect_write(mut self, bytes: &[u8]) -> Self {
        self.script.push_back(Step::Expect(bytes.to_vec()));
        self
    }

    /// Appends a reply, readable right after the previous expected write.
    #[must_use]
    pub fn then_reply(self, bytes: &[u8]) -> Self {
        self.then_reply_after(Duration::ZERO, bytes)
    }

    /// Appends a reply readable `delay` after the previous reply (or after the
    /// previous expected write, if later).
    #[must_use]
    pub fn then_reply_after(mut self, delay: Duration, bytes: &[u8]) -> Self {
        self.script.push_back(Step::Reply {
            delay,
            bytes: bytes.to_vec(),
        });
        self
    }

    /// Makes `bytes` readable immediately, outside the script.
    pub fn push_incoming(&mut self, bytes: &[u8]) {
        self.incoming.extend(bytes);
    }

    /// Every byte written so far.
    #[must_use]
    pub fn written(&self) -> &[u8] {
        &self.written
    }

    /// `true` when the whole script was consumed and every reply was read.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.script.is_empty() && self.pending.is_empty() && self.incoming.is_empty()
    }

    /// Schedules the replies at the front of the script.
    fn schedule_replies(&mut self) {
        let now = self.clock.now();
        while let Some(Step::Reply { .. }) = self.script.front() {
            if let Some(Step::Reply { delay, bytes }) = self.script.pop_front() {
                let due = self.last_due.max(now).saturating_add(delay);
                self.last_due = due;
                self.pending.push_back((due, bytes));
            }
        }
    }

    /// Moves the replies that are due at `now` to the readable buffer.
    fn release_due(&mut self) {
        let now = self.clock.now();
        while self.pending.front().is_some_and(|(due, _)| *due <= now) {
            if let Some((_, bytes)) = self.pending.pop_front() {
                self.incoming.extend(bytes);
            }
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 3);
    for (i, byte) in bytes.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let _ = write!(out, "{byte:02X}");
    }
    out
}

impl Transport for MemoryTransport {
    fn write_all(&mut self, bytes: &[u8]) -> Result<()> {
        self.schedule_replies();
        self.written.extend_from_slice(bytes);
        match self.script.pop_front() {
            Some(Step::Expect(expected)) if expected == bytes => {
                self.schedule_replies();
                Ok(())
            }
            Some(Step::Expect(expected)) => Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "unexpected write: expected [{}], got [{}]",
                    hex(&expected),
                    hex(bytes)
                ),
            ))),
            _ => Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "unexpected write after the end of the script: [{}]",
                    hex(bytes)
                ),
            ))),
        }
    }

    fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize> {
        self.schedule_replies();
        self.release_due();
        if self.incoming.is_empty() {
            let deadline = self.clock.now().saturating_add(timeout);
            match self.pending.front() {
                Some((due, _)) if *due <= deadline => {
                    self.clock.advance(due.saturating_sub(self.clock.now()));
                    self.release_due();
                }
                _ => {
                    self.clock.advance(timeout);
                    return Ok(0);
                }
            }
        }
        let mut count = 0;
        for slot in buf.iter_mut() {
            match self.incoming.pop_front() {
                Some(byte) => {
                    *slot = byte;
                    count += 1;
                }
                None => break,
            }
        }
        Ok(count)
    }

    fn discard_input(&mut self) -> Result<()> {
        self.release_due();
        self.incoming.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies_follow_expected_writes() {
        let clock = ManualClock::new();
        let mut transport = MemoryTransport::new(clock.clone())
            .expect_write(b"ab")
            .then_reply(b"xy");
        let mut buf = [0; 8];
        assert_eq!(
            transport
                .read_timeout(&mut buf, Duration::from_secs(1))
                .unwrap(),
            0
        );
        assert_eq!(clock.now(), Duration::from_secs(1));
        transport.write_all(b"ab").unwrap();
        assert_eq!(
            transport
                .read_timeout(&mut buf[..1], Duration::from_secs(1))
                .unwrap(),
            1
        );
        assert_eq!(
            transport
                .read_timeout(&mut buf, Duration::from_secs(1))
                .unwrap(),
            1
        );
        assert_eq!(&buf[..1], b"y");
        assert_eq!(clock.now(), Duration::from_secs(1));
        assert!(transport.is_finished());
        assert_eq!(transport.written(), b"ab");
    }

    #[test]
    fn delayed_replies_advance_the_clock() {
        let clock = ManualClock::new();
        let mut transport = MemoryTransport::new(clock.clone())
            .then_reply_after(Duration::from_secs(10), b"a")
            .then_reply_after(Duration::from_secs(10), b"b");
        let mut buf = [0; 8];
        assert_eq!(
            transport
                .read_timeout(&mut buf, Duration::from_secs(5))
                .unwrap(),
            0
        );
        assert_eq!(
            transport
                .read_timeout(&mut buf, Duration::from_secs(60))
                .unwrap(),
            1
        );
        assert_eq!(clock.now(), Duration::from_secs(10));
        assert_eq!(
            transport
                .read_timeout(&mut buf, Duration::from_secs(60))
                .unwrap(),
            1
        );
        assert_eq!(clock.now(), Duration::from_secs(20));
    }

    #[test]
    fn mismatched_write_is_invalid_data() {
        let mut transport = MemoryTransport::new(ManualClock::new()).expect_write(&[0x02, 0x30]);
        let err = transport.write_all(&[0x02, 0x31]).unwrap_err();
        let Error::Io(err) = err else {
            panic!("expected Io, got {err:?}");
        };
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("expected [02 30], got [02 31]"));
        assert!(transport.write_all(b"x").is_err(), "script ended");
    }

    #[test]
    fn discard_input_drops_readable_bytes_only() {
        let clock = ManualClock::new();
        let mut transport =
            MemoryTransport::new(clock.clone()).then_reply_after(Duration::from_secs(1), b"late");
        transport.push_incoming(b"old");
        transport.discard_input().unwrap();
        let mut buf = [0; 8];
        assert_eq!(
            transport
                .read_timeout(&mut buf, Duration::from_secs(2))
                .unwrap(),
            4
        );
        assert_eq!(&buf[..4], b"late");
    }

    #[test]
    fn manual_clock_saturates() {
        let clock = ManualClock::new();
        clock.advance(Duration::MAX);
        clock.advance(Duration::from_secs(1));
        assert_eq!(clock.now(), Duration::MAX);
    }
}
