//! Longitudinal redundancy check (PROTO-02).

use super::ETX;

/// LRC of a frame: XOR of every payload byte and of the trailing `ETX`.
///
/// This is the only LRC implementation of the crate; the encoder, the decoder and
/// the simulator use it.
///
/// ```
/// use transbank_pos::protocol::lrc;
///
/// assert_eq!(lrc(b"0100"), 0x02);
/// assert_eq!(lrc(b"0500||"), 0x06);
/// ```
#[must_use]
pub fn lrc(payload: &[u8]) -> u8 {
    payload.iter().fold(ETX, |acc, byte| acc ^ byte)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_payload_is_etx() {
        assert_eq!(lrc(b""), ETX);
    }

    #[test]
    fn spec_vectors() {
        let vectors: [(&[u8], u8); 9] = [
            (b"0100", 0x02),
            (b"0300", 0x00),
            (b"0800", 0x0B),
            (b"0500||", 0x06),
            (b"0700||", 0x04),
            (b"0250|", 0x78),
            (b"0260|1|", 0x36),
            (b"1200|000142|", 0x07),
            (b"0200|000012000|ABC123||0|0", 0x3E),
        ];
        for (payload, expected) in vectors {
            assert_eq!(
                lrc(payload),
                expected,
                "{}",
                String::from_utf8_lossy(payload)
            );
        }
    }
}
