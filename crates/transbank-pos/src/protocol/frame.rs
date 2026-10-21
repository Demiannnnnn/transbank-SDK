//! Frame encoding and text decoding (ADR-06).

use super::{ETX, MAX_PAYLOAD, STX, lrc};
use crate::error::{Error, Result};

/// Builds the frame `STX payload ETX LRC` for `payload`.
///
/// Only printable ASCII (`0x20..=0x7E`) is ever sent to the terminal.
///
/// # Errors
///
/// - [`Error::InvalidArgument`] if `payload` is empty or contains a byte outside
///   printable ASCII (control bytes, `DEL` or any non-ASCII character).
/// - [`Error::Overflow`] if `payload` is longer than [`MAX_PAYLOAD`] bytes.
///
/// ```
/// use transbank_pos::protocol::encode_frame;
///
/// let frame = encode_frame("0500||")?;
/// assert_eq!(frame, [0x02, b'0', b'5', b'0', b'0', b'|', b'|', 0x03, 0x06]);
/// assert!(encode_frame("").is_err());
/// # Ok::<(), transbank_pos::Error>(())
/// ```
pub fn encode_frame(payload: &str) -> Result<Vec<u8>> {
    let bytes = payload.as_bytes();
    if bytes.is_empty() {
        return Err(Error::InvalidArgument {
            field: "payload",
            reason: "must not be empty",
        });
    }
    if bytes.len() > MAX_PAYLOAD {
        return Err(Error::Overflow { max: MAX_PAYLOAD });
    }
    if !bytes.iter().all(|byte| (0x20..=0x7E).contains(byte)) {
        return Err(Error::InvalidArgument {
            field: "payload",
            reason: "must be printable ASCII",
        });
    }
    let mut frame = Vec::with_capacity(bytes.len() + 3);
    frame.push(STX);
    frame.extend_from_slice(bytes);
    frame.push(ETX);
    frame.push(lrc(bytes));
    Ok(frame)
}

/// Decodes received bytes as ISO-8859-1 (each byte is the Unicode character of the
/// same value). Never fails and keeps the accents of vouchers.
///
/// ```
/// use transbank_pos::protocol::decode_latin1;
///
/// assert_eq!(decode_latin1(b"0210|00"), "0210|00");
/// assert_eq!(decode_latin1(&[b'A', 0xD1, b'O']), "AÑO");
/// ```
#[must_use]
pub fn decode_latin1(bytes: &[u8]) -> String {
    bytes.iter().copied().map(char::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect()
    }

    #[test]
    fn spec_vectors_byte_by_byte() {
        let vectors = [
            ("0100", "02 30 31 30 30 03 02"),
            ("0300", "02 30 33 30 30 03 00"),
            ("0800", "02 30 38 30 30 03 0B"),
            ("0500||", "02 30 35 30 30 7C 7C 03 06"),
            ("0700||", "02 30 37 30 30 7C 7C 03 04"),
            ("0250|", "02 30 32 35 30 7C 03 78"),
            ("0260|1|", "02 30 32 36 30 7C 31 7C 03 36"),
            (
                "1200|000142|",
                "02 31 32 30 30 7C 30 30 30 31 34 32 7C 03 07",
            ),
            (
                "0200|000012000|ABC123||0|0",
                "02 30 32 30 30 7C 30 30 30 30 31 32 30 30 30 7C 41 42 43 31 32 33 7C 7C 30 7C 30 03 3E",
            ),
        ];
        for (payload, frame) in vectors {
            assert_eq!(encode_frame(payload).unwrap(), hex(frame), "{payload}");
        }
    }

    #[test]
    fn rejects_invalid_payloads() {
        for payload in ["", "01\u{2}00", "01\u{3}00", "AÑO", "0100\n", "01\u{7f}"] {
            assert!(
                matches!(
                    encode_frame(payload),
                    Err(Error::InvalidArgument {
                        field: "payload",
                        ..
                    })
                ),
                "{payload:?}"
            );
        }
        let long = "1".repeat(MAX_PAYLOAD + 1);
        assert!(matches!(
            encode_frame(&long),
            Err(Error::Overflow { max: MAX_PAYLOAD })
        ));
    }

    #[test]
    fn accepts_max_payload() {
        let max = "1".repeat(MAX_PAYLOAD);
        assert_eq!(encode_frame(&max).unwrap().len(), MAX_PAYLOAD + 3);
    }

    #[test]
    fn latin1_maps_every_byte() {
        let all: Vec<u8> = (0..=255).collect();
        let text = decode_latin1(&all);
        assert_eq!(text.chars().count(), 256);
        assert!(text.chars().zip(0u32..).all(|(c, i)| u32::from(c) == i));
    }
}
