//! [`TcpServer`]: serves a [`PosSimulator`] over TCP (`docs/simulator-spec.md` §5).

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, ToSocketAddrs};
use std::time::Duration;

use transbank_pos::protocol::STX;
use transbank_pos::{Error, Result};

use crate::simulator::PosSimulator;

/// TCP server for one client at a time (a POS serves a single checkout).
///
/// Listens only on loopback addresses unless `allow_remote` is set (SEC-04).
///
/// ```no_run
/// use transbank_pos_sim::{PosSimulator, TcpServer, TerminalKind};
///
/// let mut server = TcpServer::bind("127.0.0.1:7777", false)?;
/// let mut sim = PosSimulator::new(TerminalKind::Integrado);
/// server.serve_one(&mut sim)?; // until the client disconnects
/// # Ok::<(), transbank_pos::Error>(())
/// ```
#[derive(Debug)]
pub struct TcpServer {
    listener: TcpListener,
    delay: Duration,
}

impl TcpServer {
    /// Binds `addr`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] if `addr` is not a loopback address and
    /// `allow_remote` is `false`; [`Error::Io`] if it cannot be resolved or bound.
    pub fn bind(addr: impl ToSocketAddrs, allow_remote: bool) -> Result<Self> {
        let addrs: Vec<SocketAddr> = addr.to_socket_addrs()?.collect();
        if !allow_remote && addrs.iter().any(|addr| !addr.ip().is_loopback()) {
            return Err(Error::InvalidArgument {
                field: "listen",
                reason: "non-loopback address requires allow_remote",
            });
        }
        Ok(Self {
            listener: TcpListener::bind(addrs.as_slice())?,
            delay: Duration::ZERO,
        })
    }

    /// Waits `delay` before writing each response frame (for demos).
    #[must_use]
    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    /// Bound address (useful with port 0).
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if the address cannot be read.
    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.listener.local_addr()?)
    }

    /// Accepts one connection and serves it until the client closes it.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] on accept, read or write errors.
    pub fn serve_one(&mut self, sim: &mut PosSimulator) -> Result<()> {
        self.serve_one_with(sim, &mut |_, _| {})
    }

    /// Like [`serve_one`](Self::serve_one), calling `observer(input, output)` for
    /// every chunk received from the host and the bytes sent back.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] on accept, read or write errors.
    pub fn serve_one_with(
        &mut self,
        sim: &mut PosSimulator,
        observer: &mut dyn FnMut(&[u8], &[u8]),
    ) -> Result<()> {
        let (mut stream, _) = self.listener.accept()?;
        stream.set_nodelay(true)?;
        let mut buf = [0u8; 1024];
        loop {
            let read = stream.read(&mut buf)?;
            if read == 0 {
                return Ok(());
            }
            let input = buf.get(..read).unwrap_or_default();
            let output = sim.feed(input);
            observer(input, &output);
            if !self.delay.is_zero() && output.contains(&STX) {
                std::thread::sleep(self.delay);
            }
            stream.write_all(&output)?;
        }
    }

    /// Serves clients one after another, forever.
    ///
    /// # Errors
    ///
    /// The first [`Error::Io`] of [`serve_one`](Self::serve_one).
    pub fn run(mut self, sim: &mut PosSimulator) -> Result<()> {
        loop {
            self.serve_one(sim)?;
        }
    }
}
