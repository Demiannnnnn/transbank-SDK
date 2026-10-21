//! [`TcpTransport`]: TCP connection to a serial-to-network bridge or to the simulator.

use std::io::{ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use super::Transport;
use crate::error::{Error, Result};

/// Shortest read timeout passed to the socket (a zero timeout is invalid for it).
const MIN_READ_TIMEOUT: Duration = Duration::from_millis(1);

/// Transport over a TCP connection (serial bridges, `tbk-pos-sim`).
///
/// It only connects to the address given by the application; it never listens.
///
/// ```no_run
/// use std::time::Duration;
/// use transbank_pos::PosIntegrado;
/// use transbank_pos::transport::TcpTransport;
///
/// let transport = TcpTransport::connect("127.0.0.1:7777", Duration::from_secs(2))?;
/// let mut pos = PosIntegrado::new(transport);
/// pos.poll()?;
/// # Ok::<(), transbank_pos::Error>(())
/// ```
#[derive(Debug)]
pub struct TcpTransport {
    stream: TcpStream,
}

impl TcpTransport {
    /// Connects to `addr`, trying each resolved address with `connect_timeout`.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if the address cannot be resolved or no address accepts the
    /// connection (the last error is returned).
    pub fn connect(addr: impl ToSocketAddrs, connect_timeout: Duration) -> Result<Self> {
        let mut last_error =
            std::io::Error::new(ErrorKind::InvalidInput, "address resolved to nothing");
        for addr in addr.to_socket_addrs()? {
            match TcpStream::connect_timeout(&addr, connect_timeout) {
                Ok(stream) => return Self::from_stream(stream),
                Err(err) => last_error = err,
            }
        }
        Err(Error::Io(last_error))
    }

    /// Wraps an already connected stream.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if `TCP_NODELAY` cannot be set.
    pub fn from_stream(stream: TcpStream) -> Result<Self> {
        stream.set_nodelay(true)?;
        Ok(Self { stream })
    }
}

impl Transport for TcpTransport {
    fn write_all(&mut self, bytes: &[u8]) -> Result<()> {
        self.stream.write_all(bytes)?;
        self.stream.flush()?;
        Ok(())
    }

    fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize> {
        self.stream
            .set_read_timeout(Some(timeout.max(MIN_READ_TIMEOUT)))?;
        match self.stream.read(buf) {
            Ok(0) if !buf.is_empty() => Err(Error::Io(std::io::Error::new(
                ErrorKind::UnexpectedEof,
                "connection closed by the peer",
            ))),
            Ok(read) => Ok(read),
            Err(err) if matches!(err.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => Ok(0),
            Err(err) if err.kind() == ErrorKind::Interrupted => Ok(0),
            Err(err) => Err(Error::Io(err)),
        }
    }

    fn discard_input(&mut self) -> Result<()> {
        self.stream.set_nonblocking(true)?;
        let mut buf = [0u8; 512];
        let result = loop {
            match self.stream.read(&mut buf) {
                Ok(0) => break Ok(()),
                Ok(_) => {}
                Err(err) if err.kind() == ErrorKind::WouldBlock => break Ok(()),
                Err(err) if err.kind() == ErrorKind::Interrupted => {}
                Err(err) => break Err(Error::Io(err)),
            }
        };
        self.stream.set_nonblocking(false)?;
        result
    }
}
