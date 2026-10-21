//! [`SimTransport`]: the simulator behind the `Transport` trait (`docs/simulator-spec.md` §4).

use std::collections::VecDeque;
use std::time::Duration;

use transbank_pos::Transport;
use transbank_pos::transport::ManualClock;

use crate::simulator::PosSimulator;

/// In-memory transport connected to a [`PosSimulator`], with simulated time.
///
/// `write_all` feeds the simulator and queues its output; `read_timeout` returns the
/// queued bytes or, if there are none, advances the [`ManualClock`] by the timeout
/// and returns `Ok(0)`.
///
/// ```
/// use transbank_pos::transport::ManualClock;
/// use transbank_pos::{Link, LinkConfig};
/// use transbank_pos_sim::{PosSimulator, SimTransport, TerminalKind};
///
/// let clock = ManualClock::new();
/// let transport = SimTransport::new(PosSimulator::new(TerminalKind::Integrado), clock.clone());
/// let mut link = Link::with_clock(transport, LinkConfig::default(), clock);
/// let response = link.request("0700||", "0710", &mut |_| {})?;
/// assert_eq!(response, "0710|00|000|0||");
/// # Ok::<(), transbank_pos::Error>(())
/// ```
#[derive(Debug)]
pub struct SimTransport {
    sim: PosSimulator,
    clock: ManualClock,
    incoming: VecDeque<u8>,
}

impl SimTransport {
    /// Connects `sim` with simulated time driven by `clock`.
    #[must_use]
    pub fn new(sim: PosSimulator, clock: ManualClock) -> Self {
        Self {
            sim,
            clock,
            incoming: VecDeque::new(),
        }
    }

    /// The simulator (to change the scenario between operations).
    pub fn sim_mut(&mut self) -> &mut PosSimulator {
        &mut self.sim
    }

    /// The simulator.
    #[must_use]
    pub fn sim(&self) -> &PosSimulator {
        &self.sim
    }

    /// Returns the simulator.
    #[must_use]
    pub fn into_inner(self) -> PosSimulator {
        self.sim
    }
}

impl Transport for SimTransport {
    fn write_all(&mut self, bytes: &[u8]) -> transbank_pos::Result<()> {
        let output = self.sim.feed(bytes);
        self.incoming.extend(output);
        Ok(())
    }

    fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> transbank_pos::Result<usize> {
        if self.incoming.is_empty() {
            self.clock.advance(timeout);
            return Ok(0);
        }
        let mut count = 0;
        for slot in buf.iter_mut() {
            let Some(byte) = self.incoming.pop_front() else {
                break;
            };
            *slot = byte;
            count += 1;
        }
        Ok(count)
    }

    fn discard_input(&mut self) -> transbank_pos::Result<()> {
        self.incoming.clear();
        Ok(())
    }
}
