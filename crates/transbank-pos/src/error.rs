//! Error type of the crate. A declined card is a successful response, never an `Error`.

use std::time::Duration;

/// Result type of the crate, with [`Error`] as the default error.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Communication or protocol failure.
///
/// A declined card or any other business rejection reported by the terminal is
/// **not** an `Error`: it is returned as a successful response whose response code
/// says so (ADR-03).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// I/O error of the underlying transport.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Transport error that is not an [`std::io::Error`] (for example, from `serialport`).
    #[error("transport error: {0}")]
    Transport(Box<dyn std::error::Error + Send + Sync>),
    /// The frame was not acknowledged after `attempts` transmissions.
    #[error("POS did not acknowledge the command after {attempts} attempts ({naks} NAK)")]
    NoAck {
        /// Number of transmissions of the frame.
        attempts: u8,
        /// How many of those transmissions were answered with NAK.
        naks: u8,
    },
    /// No response arrived within the response timeout.
    ///
    /// For a sale the outcome is **unknown**: query the last sale before retrying.
    #[error("no response from POS within {waited:?}")]
    ResponseTimeout {
        /// Time waited for the response.
        waited: Duration,
    },
    /// Too many consecutive frames with an invalid LRC.
    #[error("received {count} frames with invalid LRC")]
    BadLrc {
        /// Number of consecutive frames with an invalid LRC.
        count: u8,
    },
    /// The terminal answered with a different function code than expected.
    #[error("expected response {expected}, received {received}")]
    UnexpectedResponse {
        /// Expected function code.
        expected: &'static str,
        /// Function code received.
        received: String,
        /// Raw payload received.
        payload: String,
    },
    /// A field of a response has an invalid value.
    #[error("invalid value {value:?} in field {field}")]
    Parse {
        /// Name of the field.
        field: &'static str,
        /// Raw value of the field.
        value: String,
        /// Raw payload of the response.
        payload: String,
    },
    /// An argument is invalid; nothing was sent to the terminal.
    #[error("invalid argument {field}: {reason}")]
    InvalidArgument {
        /// Name of the argument.
        field: &'static str,
        /// Why it is invalid.
        reason: &'static str,
    },
    /// A frame exceeds the maximum payload size.
    #[error("frame larger than {max} bytes")]
    Overflow {
        /// Maximum payload size in bytes.
        max: usize,
    },
    /// A multi-frame response has more frames than allowed.
    #[error("more than {max} frames in a multi-frame response")]
    TooManyFrames {
        /// Maximum number of frames.
        max: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync_static<T: Send + Sync + 'static>() {}

    #[test]
    fn error_is_send_sync_static() {
        assert_send_sync_static::<Error>();
    }

    #[test]
    fn io_error_converts() {
        let err: Error = std::io::Error::from(std::io::ErrorKind::UnexpectedEof).into();
        assert!(matches!(err, Error::Io(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof));
    }

    #[test]
    fn messages_are_informative() {
        let err = Error::NoAck {
            attempts: 3,
            naks: 1,
        };
        assert_eq!(
            err.to_string(),
            "POS did not acknowledge the command after 3 attempts (1 NAK)"
        );
        let err = Error::InvalidArgument {
            field: "amount",
            reason: "out of range",
        };
        assert_eq!(err.to_string(), "invalid argument amount: out of range");
    }
}
