//! Byte channels to the terminal ([`Transport`]), injectable clocks ([`Clock`]), the
//! TCP transport and the in-memory transport for tests. The serial port transport is
//! in `transbank_pos::serial` (feature `serial`).

mod memory;
mod tcp;

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crate::error::Result;

pub use memory::{ManualClock, MemoryTransport};
pub use tcp::TcpTransport;

/// Byte channel to the POS (serial port, TCP bridge, memory…).
///
/// The link writes whole frames or single control bytes with
/// [`write_all`](Transport::write_all) and polls with
/// [`read_timeout`](Transport::read_timeout).
pub trait Transport: Send {
    /// Writes every byte.
    ///
    /// # Errors
    ///
    /// Any I/O or transport error.
    fn write_all(&mut self, bytes: &[u8]) -> Result<()>;

    /// Waits up to `timeout` for bytes and copies them into `buf`.
    ///
    /// Returns `Ok(0)` if the timeout elapsed without data. Never blocks longer than
    /// `timeout`.
    ///
    /// # Errors
    ///
    /// Any I/O or transport error (a closed connection is
    /// [`Error::Io`](crate::Error::Io) with `UnexpectedEof`).
    fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize>;

    /// Discards pending input before a command is sent. Does nothing by default.
    ///
    /// # Errors
    ///
    /// Any I/O or transport error.
    fn discard_input(&mut self) -> Result<()> {
        Ok(())
    }
}

impl<T: Transport + ?Sized> Transport for &mut T {
    fn write_all(&mut self, bytes: &[u8]) -> Result<()> {
        (**self).write_all(bytes)
    }

    fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize> {
        (**self).read_timeout(buf, timeout)
    }

    fn discard_input(&mut self) -> Result<()> {
        (**self).discard_input()
    }
}

impl<T: Transport + ?Sized> Transport for Box<T> {
    fn write_all(&mut self, bytes: &[u8]) -> Result<()> {
        (**self).write_all(bytes)
    }

    fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize> {
        (**self).read_timeout(buf, timeout)
    }

    fn discard_input(&mut self) -> Result<()> {
        (**self).discard_input()
    }
}

/// Monotonic clock used by the link for timeouts (ADR-09).
///
/// Production code uses [`SystemClock`]; tests use [`ManualClock`] together with
/// [`MemoryTransport`] so timeouts take no real time.
pub trait Clock: Send {
    /// Time elapsed since an arbitrary fixed origin.
    fn now(&self) -> Duration;
}

/// Real monotonic clock ([`Instant`]-based).
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Duration {
        static ORIGIN: OnceLock<Instant> = OnceLock::new();
        ORIGIN.get_or_init(Instant::now).elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_clock_is_monotonic() {
        let clock = SystemClock;
        let first = clock.now();
        assert!(clock.now() >= first);
    }

    #[test]
    fn references_and_boxes_are_transports() {
        fn assert_transport<T: Transport>() {}
        assert_transport::<&mut MemoryTransport>();
        assert_transport::<Box<dyn Transport>>();
    }
}
