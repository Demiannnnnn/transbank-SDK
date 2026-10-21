//! Parser of the host requests, from the POS side.

use transbank_pos::{Error, Result};

/// Arguments of a sale (`0200`) or multi-code sale (`0270`) as received.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SaleArgs {
    /// Amount in CLP (with or without zero padding on the wire).
    pub amount: u64,
    /// Ticket exactly as received (it may be padded with zeros or too long).
    pub ticket: String,
    /// The host asked for the voucher.
    pub voucher: bool,
    /// The host asked for intermediate `0900` messages.
    pub status: bool,
}

/// A host request understood by the simulator.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SimRequest {
    /// `0100`.
    Poll,
    /// `0300`.
    NormalMode,
    /// `0800`.
    LoadKeys,
    /// `0200`.
    Sale(SaleArgs),
    /// `0270`.
    MultiCodeSale {
        /// Sale arguments.
        sale: SaleArgs,
        /// Commerce code of the child commerce (`"0"` = main).
        commerce_code: String,
    },
    /// `0250`.
    LastSale,
    /// `0280`.
    MultiCodeLastSale {
        /// The host asked for the voucher.
        voucher: bool,
    },
    /// `1200`.
    Refund {
        /// Operation number of the sale to refund.
        operation: u32,
    },
    /// `0700`.
    Totals,
    /// `0260`.
    SalesDetail {
        /// `true` = send each sale to the host (`1`); `false` = print on the POS (`0`).
        send_to_host: bool,
    },
    /// `0290`.
    MultiCodeSalesDetail {
        /// `true` = send each sale to the host (`1`); `false` = print on the POS (`0`).
        send_to_host: bool,
    },
    /// `0500`.
    Close,
    /// Any other function code, as received.
    Unknown(String),
}

fn text<'a>(fields: &[&'a str], index: usize) -> &'a str {
    fields.get(index).map_or("", |value| value.trim())
}

fn number<T: std::str::FromStr>(
    fields: &[&str],
    index: usize,
    name: &'static str,
    payload: &str,
) -> Result<T> {
    let value = text(fields, index);
    let parsed = if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) {
        value.parse().ok()
    } else {
        None
    };
    parsed.ok_or_else(|| Error::Parse {
        field: name,
        value: value.to_owned(),
        payload: payload.to_owned(),
    })
}

fn sale_args(fields: &[&str], payload: &str) -> Result<SaleArgs> {
    Ok(SaleArgs {
        amount: number(fields, 1, "amount", payload)?,
        ticket: text(fields, 2).to_owned(),
        voucher: text(fields, 4) == "1",
        status: text(fields, 5) == "1",
    })
}

/// Parses a host request.
///
/// Accepts the three sale variants of `docs/protocol-spec.md` OPEN-01: amount with or
/// without zero padding, empty or explicit voucher field, with or without a trailing
/// `|` (`0200|000012000|ABC123|||0`, `0200|12000|ABC123||0|1|`,
/// `0200|000012000|ABC123||0|0`).
///
/// # Errors
///
/// [`Error::Parse`] if a numeric field (amount, operation) is missing or invalid.
///
/// ```
/// use transbank_pos_sim::{SimRequest, parse_request};
///
/// let SimRequest::Sale(sale) = parse_request("0200|12000|ABC123||0|1|")? else {
///     unreachable!()
/// };
/// assert_eq!((sale.amount, sale.ticket.as_str(), sale.status), (12_000, "ABC123", true));
/// # Ok::<(), transbank_pos::Error>(())
/// ```
pub fn parse_request(payload: &str) -> Result<SimRequest> {
    let fields: Vec<&str> = payload.split('|').collect();
    let request = match text(&fields, 0) {
        "0100" => SimRequest::Poll,
        "0300" => SimRequest::NormalMode,
        "0800" => SimRequest::LoadKeys,
        "0200" => SimRequest::Sale(sale_args(&fields, payload)?),
        "0270" => SimRequest::MultiCodeSale {
            sale: sale_args(&fields, payload)?,
            commerce_code: text(&fields, 6).to_owned(),
        },
        "0250" => SimRequest::LastSale,
        "0280" => SimRequest::MultiCodeLastSale {
            voucher: text(&fields, 1) == "1",
        },
        "1200" => SimRequest::Refund {
            operation: number(&fields, 1, "operation", payload)?,
        },
        "0700" => SimRequest::Totals,
        "0260" => SimRequest::SalesDetail {
            send_to_host: text(&fields, 1) == "1",
        },
        "0290" => SimRequest::MultiCodeSalesDetail {
            send_to_host: text(&fields, 1) == "1",
        },
        "0500" => SimRequest::Close,
        other => SimRequest::Unknown(other.to_owned()),
    };
    Ok(request)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sale(payload: &str) -> SaleArgs {
        match parse_request(payload).unwrap() {
            SimRequest::Sale(sale) => sale,
            other => panic!("not a sale: {other:?}"),
        }
    }

    #[test]
    fn accepts_the_three_sale_variants() {
        let node = sale("0200|000012000|ABC123|||0");
        assert_eq!(
            (node.amount, node.ticket.as_str(), node.voucher, node.status),
            (12_000, "ABC123", false, false)
        );
        let dotnet = sale("0200|12000|ABC123||0|1|");
        assert_eq!(
            (dotnet.amount, dotnet.voucher, dotnet.status),
            (12_000, false, true)
        );
        let ours = sale("0200|000012000|ABC123||1|0");
        assert_eq!(
            (ours.amount, ours.voucher, ours.status),
            (12_000, true, false)
        );
    }

    #[test]
    fn other_requests() {
        assert_eq!(parse_request("0100").unwrap(), SimRequest::Poll);
        assert_eq!(
            parse_request("1200|000142|").unwrap(),
            SimRequest::Refund { operation: 142 }
        );
        assert_eq!(
            parse_request("0260|1|").unwrap(),
            SimRequest::SalesDetail { send_to_host: true }
        );
        assert_eq!(
            parse_request("0290|0|").unwrap(),
            SimRequest::MultiCodeSalesDetail {
                send_to_host: false
            }
        );
        assert_eq!(
            parse_request("0280|1").unwrap(),
            SimRequest::MultiCodeLastSale { voucher: true }
        );
        assert_eq!(
            parse_request("0270|000005000|000007||0|0|597012345678").unwrap(),
            SimRequest::MultiCodeSale {
                sale: SaleArgs {
                    amount: 5_000,
                    ticket: "000007".into(),
                    voucher: false,
                    status: false
                },
                commerce_code: "597012345678".into(),
            }
        );
        assert_eq!(
            parse_request("9999|x").unwrap(),
            SimRequest::Unknown("9999".into())
        );
    }

    #[test]
    fn invalid_numbers_are_errors() {
        assert!(matches!(
            parse_request("0200|12a|1||0|0"),
            Err(Error::Parse {
                field: "amount",
                ..
            })
        ));
        assert!(matches!(
            parse_request("1200||"),
            Err(Error::Parse {
                field: "operation",
                ..
            })
        ));
    }
}
