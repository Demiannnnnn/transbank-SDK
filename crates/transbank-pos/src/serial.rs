//! Serial port transport, port listing and auto-detection (feature `serial`).
//!
//! The POS appears as a USB CDC-ACM serial port (`COMx`, `/dev/ttyACM*`,
//! `/dev/cu.usbmodem*`). Format 8N1 without flow control (`docs/protocol-spec.md` §1).

use std::io::{ErrorKind, Read, Write};
use std::time::Duration;

use serialport::{
    ClearBuffer, DataBits, FlowControl, Parity, SerialPort, SerialPortType, StopBits,
};

use crate::client::PosIntegrado;
use crate::error::{Error, Result};
use crate::transport::Transport;

/// Default baud rate of the POS Integrado.
pub const DEFAULT_BAUD_INTEGRADO: u32 = 115_200;
/// Default baud rate of the POS Autoservicio.
pub const DEFAULT_BAUD_AUTOSERVICIO: u32 = 19_200;
/// Baud rates supported by the terminals.
pub const SUPPORTED_BAUD_RATES: [u32; 6] = [1_200, 2_400, 4_800, 9_600, 19_200, 115_200];

/// Shortest read timeout passed to the port.
const MIN_READ_TIMEOUT: Duration = Duration::from_millis(1);

fn serial_error(err: serialport::Error) -> Error {
    match err.kind {
        serialport::ErrorKind::Io(kind) => Error::Io(std::io::Error::new(kind, err.description)),
        serialport::ErrorKind::NoDevice
        | serialport::ErrorKind::InvalidInput
        | serialport::ErrorKind::Unknown => Error::Transport(Box::new(err)),
    }
}

/// Transport over a serial port. The port is closed when the transport is dropped.
///
/// ```no_run
/// use transbank_pos::PosIntegrado;
/// use transbank_pos::serial::{DEFAULT_BAUD_INTEGRADO, SerialTransport};
///
/// let transport = SerialTransport::open("/dev/ttyACM0", DEFAULT_BAUD_INTEGRADO)?;
/// let mut pos = PosIntegrado::new(transport);
/// pos.poll()?;
/// # Ok::<(), transbank_pos::Error>(())
/// ```
pub struct SerialTransport {
    port: Box<dyn SerialPort>,
}

impl std::fmt::Debug for SerialTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SerialTransport")
            .field("port", &self.port.name())
            .finish()
    }
}

impl SerialTransport {
    /// Opens `path` with 8N1, no flow control.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] if `baud_rate` is not in [`SUPPORTED_BAUD_RATES`]
    /// (checked before opening anything); [`Error::Io`] or [`Error::Transport`] if the
    /// port cannot be opened.
    pub fn open(path: &str, baud_rate: u32) -> Result<Self> {
        if !SUPPORTED_BAUD_RATES.contains(&baud_rate) {
            return Err(Error::InvalidArgument {
                field: "baud_rate",
                reason: "must be 1200, 2400, 4800, 9600, 19200 or 115200",
            });
        }
        let port = serialport::new(path, baud_rate)
            .data_bits(DataBits::Eight)
            .parity(Parity::None)
            .stop_bits(StopBits::One)
            .flow_control(FlowControl::None)
            .timeout(MIN_READ_TIMEOUT)
            .open()
            .map_err(serial_error)?;
        Ok(Self { port })
    }
}

impl Transport for SerialTransport {
    fn write_all(&mut self, bytes: &[u8]) -> Result<()> {
        self.port.write_all(bytes)?;
        self.port.flush()?;
        Ok(())
    }

    fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize> {
        self.port
            .set_timeout(timeout.max(MIN_READ_TIMEOUT))
            .map_err(serial_error)?;
        match self.port.read(buf) {
            Ok(read) => Ok(read),
            Err(err)
                if matches!(
                    err.kind(),
                    ErrorKind::TimedOut | ErrorKind::WouldBlock | ErrorKind::Interrupted
                ) =>
            {
                Ok(0)
            }
            Err(err) => Err(Error::Io(err)),
        }
    }

    fn discard_input(&mut self) -> Result<()> {
        self.port.clear(ClearBuffer::Input).map_err(serial_error)
    }
}

/// How a serial port is connected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub enum PortKind {
    /// USB (the POS uses USB CDC-ACM).
    Usb,
    /// PCI (built-in port).
    Pci,
    /// Bluetooth.
    Bluetooth,
    /// Unknown.
    Unknown,
}

/// A serial port of the system.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct PortInfo {
    /// Name to open (`COM3`, `/dev/ttyACM0`, `/dev/cu.usbmodem1101`…).
    pub name: String,
    /// How the port is connected.
    pub kind: PortKind,
    /// USB vendor id.
    pub vid: Option<u16>,
    /// USB product id.
    pub pid: Option<u16>,
    /// USB product name.
    pub product: Option<String>,
}

impl PortInfo {
    fn from_serialport(info: serialport::SerialPortInfo) -> Self {
        let (kind, vid, pid, product) = match info.port_type {
            SerialPortType::UsbPort(usb) => {
                (PortKind::Usb, Some(usb.vid), Some(usb.pid), usb.product)
            }
            SerialPortType::PciPort => (PortKind::Pci, None, None, None),
            SerialPortType::BluetoothPort => (PortKind::Bluetooth, None, None, None),
            SerialPortType::Unknown => (PortKind::Unknown, None, None, None),
        };
        Self {
            name: info.port_name,
            kind,
            vid,
            pid,
            product,
        }
    }
}

/// On macOS each device appears as `/dev/cu.X` and `/dev/tty.X`; keeps only `cu.*`.
fn without_tty_duplicates(ports: Vec<PortInfo>) -> Vec<PortInfo> {
    let callout: Vec<String> = ports
        .iter()
        .filter_map(|port| port.name.strip_prefix("/dev/cu.").map(str::to_owned))
        .collect();
    ports
        .into_iter()
        .filter(|port| {
            port.name
                .strip_prefix("/dev/tty.")
                .is_none_or(|rest| !callout.iter().any(|name| name == rest))
        })
        .collect()
}

/// Serial ports of the system (on macOS, only the `cu.*` of each device).
///
/// # Errors
///
/// [`Error::Io`] or [`Error::Transport`] if the ports cannot be enumerated.
pub fn list_ports() -> Result<Vec<PortInfo>> {
    let ports = serialport::available_ports()
        .map_err(serial_error)?
        .into_iter()
        .map(PortInfo::from_serialport)
        .collect();
    Ok(without_tty_duplicates(ports))
}

/// Order in which [`PosIntegrado::autoconnect`] tries the ports: USB first, then
/// the rest, each group by name; `tty.*` duplicates of macOS `cu.*` are removed.
///
/// ```
/// use transbank_pos::serial::{PortInfo, PortKind, order_candidates};
///
/// let port = |name: &str, kind| PortInfo { name: name.into(), kind, vid: None, pid: None, product: None };
/// let ordered = order_candidates(vec![
///     port("/dev/ttyS0", PortKind::Pci),
///     port("/dev/ttyACM0", PortKind::Usb),
/// ]);
/// assert_eq!(ordered[0].name, "/dev/ttyACM0");
/// ```
#[must_use]
pub fn order_candidates(ports: Vec<PortInfo>) -> Vec<PortInfo> {
    let mut ports = without_tty_duplicates(ports);
    ports.sort_by(|a, b| {
        (a.kind != PortKind::Usb, &a.name).cmp(&(b.kind != PortKind::Usb, &b.name))
    });
    ports
}

impl PosIntegrado<SerialTransport> {
    /// Opens the POS Integrado at `path` with 115200 baud.
    ///
    /// # Errors
    ///
    /// Those of [`SerialTransport::open`].
    pub fn open(path: &str) -> Result<Self> {
        Ok(Self::new(SerialTransport::open(
            path,
            DEFAULT_BAUD_INTEGRADO,
        )?))
    }

    /// Tries `poll` on every port of [`list_ports`] in the order of
    /// [`order_candidates`] and returns the first POS that answers. Ports that fail
    /// to open or to answer are closed and skipped.
    ///
    /// Each attempt takes up to 3 × 2 s without an answer.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] for an unsupported baud rate, or the error of
    /// [`list_ports`]. `Ok(None)` if no port answers.
    pub fn autoconnect(baud_rate: u32) -> Result<Option<(Self, PortInfo)>> {
        if !SUPPORTED_BAUD_RATES.contains(&baud_rate) {
            return Err(Error::InvalidArgument {
                field: "baud_rate",
                reason: "must be 1200, 2400, 4800, 9600, 19200 or 115200",
            });
        }
        for port in order_candidates(list_ports()?) {
            let Ok(transport) = SerialTransport::open(&port.name, baud_rate) else {
                continue;
            };
            let mut pos = Self::new(transport);
            if pos.poll().is_ok() {
                return Ok(Some((pos, port)));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn port(name: &str, kind: PortKind) -> PortInfo {
        PortInfo {
            name: name.to_owned(),
            kind,
            vid: None,
            pid: None,
            product: None,
        }
    }

    #[test]
    fn usb_first_then_by_name() {
        let ordered = order_candidates(vec![
            port("COM9", PortKind::Unknown),
            port("COM4", PortKind::Usb),
            port("COM1", PortKind::Pci),
            port("COM3", PortKind::Usb),
        ]);
        let names: Vec<_> = ordered.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["COM3", "COM4", "COM1", "COM9"]);
    }

    #[test]
    fn macos_tty_duplicates_are_removed() {
        let ordered = order_candidates(vec![
            port("/dev/tty.usbmodem1101", PortKind::Usb),
            port("/dev/cu.usbmodem1101", PortKind::Usb),
            port("/dev/tty.Bluetooth-Incoming-Port", PortKind::Unknown),
        ]);
        let names: Vec<_> = ordered.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names,
            ["/dev/cu.usbmodem1101", "/dev/tty.Bluetooth-Incoming-Port"]
        );
    }

    #[test]
    fn invalid_baud_rate_is_rejected_before_opening() {
        for baud in [0, 9_601, 57_600] {
            assert!(matches!(
                SerialTransport::open("/dev/does-not-exist", baud),
                Err(Error::InvalidArgument {
                    field: "baud_rate",
                    ..
                })
            ));
            assert!(matches!(
                PosIntegrado::autoconnect(baud),
                Err(Error::InvalidArgument {
                    field: "baud_rate",
                    ..
                })
            ));
        }
    }

    #[test]
    fn serial_errors_map_to_io_or_transport() {
        let io = serial_error(serialport::Error::new(
            serialport::ErrorKind::Io(ErrorKind::PermissionDenied),
            "denied",
        ));
        assert!(matches!(io, Error::Io(ref e) if e.kind() == ErrorKind::PermissionDenied));
        let other = serial_error(serialport::Error::new(
            serialport::ErrorKind::NoDevice,
            "busy",
        ));
        assert!(matches!(other, Error::Transport(_)));
    }

    #[test]
    fn serial_transport_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<SerialTransport>();
        assert_send::<PosIntegrado<SerialTransport>>();
    }
}
