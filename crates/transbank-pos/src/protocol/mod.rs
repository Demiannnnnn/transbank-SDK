//! Sans-IO protocol core: control bytes, LRC, frame encoding and the frame decoder.
//!
//! Nothing in this module performs I/O or reads the clock, so it can be reused by
//! the link, the simulator and any async adapter. The format is described in
//! `docs/protocol-spec.md` §2.
//!
//! ```
//! use transbank_pos::protocol::{FrameDecoder, LinkEvent, encode_frame};
//!
//! let frame = encode_frame("0100")?;
//! assert_eq!(frame, [0x02, b'0', b'1', b'0', b'0', 0x03, 0x02]);
//!
//! let mut decoder = FrameDecoder::new();
//! let mut events = Vec::new();
//! decoder.push(&frame, &mut events);
//! assert_eq!(events, [LinkEvent::Frame(b"0100".to_vec())]);
//! # Ok::<(), transbank_pos::Error>(())
//! ```

mod decoder;
mod frame;
mod lrc;

pub use decoder::{FrameDecoder, LinkEvent};
pub use frame::{decode_latin1, encode_frame};
pub use lrc::lrc;

/// Start of frame.
pub const STX: u8 = 0x02;
/// End of the frame data; the LRC byte follows.
pub const ETX: u8 = 0x03;
/// Positive acknowledgement (single byte, outside frames).
pub const ACK: u8 = 0x06;
/// Negative acknowledgement: the frame arrived corrupted (single byte, outside frames).
pub const NAK: u8 = 0x15;
/// Field separator inside a payload (`|`).
pub const FIELD_SEPARATOR: u8 = b'|';
/// Maximum payload size in bytes accepted by the encoder and the decoder.
pub const MAX_PAYLOAD: usize = 4096;
