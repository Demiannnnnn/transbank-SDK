//! Failure injection scenarios (`docs/simulator-spec.md` §2).

/// Behaviour of the simulator. The "next N" scenarios are consumed and then the
/// simulator goes back to [`Scenario::Approve`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Scenario {
    /// Everything is approved (default).
    #[default]
    Approve,
    /// The next sale answers with this response code (for example 1 rejected,
    /// 7 canceled at the POS).
    RejectNextSale(u16),
    /// `0900` codes sent before each sale response when the host asks for
    /// intermediate messages (without this scenario: 78, 79, 81, 82).
    Intermediate(Vec<u16>),
    /// Does not send ACK to the next N commands (nor processes them).
    DropAcks(u8),
    /// Answers NAK to the next N commands (and does not process them).
    NakNext(u8),
    /// Sends the ACK but never the response (a "hung" sale). Persistent.
    NoResponse,
    /// The next N transmissions of a response go out with a corrupted LRC.
    CorruptLrc(u8),
    /// Prepends these bytes to the next response.
    Garbage(Vec<u8>),
    /// The next response uses this function code instead of the right one (LINK-06).
    WrongFunctionCode(&'static str),
    /// Every sale answers 74 ("Debe Ejecutar CIERRE") until a close (`0500`) arrives.
    RequireClose,
    /// Every sale response includes a voucher of N lines of 40 characters. Persistent.
    WithVoucher(u8),
}
