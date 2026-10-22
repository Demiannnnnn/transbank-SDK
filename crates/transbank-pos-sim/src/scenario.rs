//! Failure injection scenarios (`docs/simulator-spec.md` §2).

use std::str::FromStr;

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

impl FromStr for Scenario {
    type Err = String;

    /// Parses the `--scenario` syntax of `tbk-pos-sim` and `tbk-pos --sim`:
    /// `approve`, `reject:CODE`, `intermediate`, `no-response`, `corrupt-lrc:N`,
    /// `require-close`, `drop-acks:N`, `nak:N`, `voucher:LINES`.
    ///
    /// ```
    /// use transbank_pos_sim::Scenario;
    ///
    /// assert_eq!("reject:1".parse(), Ok(Scenario::RejectNextSale(1)));
    /// assert!("reject".parse::<Scenario>().is_err());
    /// ```
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (name, argument) = text.split_once(':').unwrap_or((text, ""));
        let number = || -> Result<u8, String> {
            argument
                .parse()
                .map_err(|_| format!("el escenario {name} requiere un número (p. ej. {name}:1)"))
        };
        match name {
            "approve" => Ok(Self::Approve),
            "reject" => argument
                .parse()
                .map(Self::RejectNextSale)
                .map_err(|_| "reject requiere un código (p. ej. reject:1)".to_owned()),
            "intermediate" => Ok(Self::Intermediate(vec![78, 79, 81, 82])),
            "no-response" => Ok(Self::NoResponse),
            "corrupt-lrc" => number().map(Self::CorruptLrc),
            "require-close" => Ok(Self::RequireClose),
            "drop-acks" => number().map(Self::DropAcks),
            "nak" => number().map(Self::NakNext),
            "voucher" => number().map(Self::WithVoucher),
            other => Err(format!("escenario desconocido: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_cli_scenario() {
        let cases = [
            ("approve", Scenario::Approve),
            ("reject:7", Scenario::RejectNextSale(7)),
            ("intermediate", Scenario::Intermediate(vec![78, 79, 81, 82])),
            ("no-response", Scenario::NoResponse),
            ("corrupt-lrc:2", Scenario::CorruptLrc(2)),
            ("require-close", Scenario::RequireClose),
            ("drop-acks:3", Scenario::DropAcks(3)),
            ("nak:1", Scenario::NakNext(1)),
            ("voucher:4", Scenario::WithVoucher(4)),
        ];
        for (text, scenario) in cases {
            assert_eq!(text.parse::<Scenario>(), Ok(scenario), "{text}");
        }
        for bad in ["", "reject", "nak:x", "corrupt-lrc:300", "other"] {
            assert!(bad.parse::<Scenario>().is_err(), "{bad}");
        }
    }
}
