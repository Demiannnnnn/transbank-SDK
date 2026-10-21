//! POS Integrado requests and their payloads (`docs/protocol-spec.md` §4).

use crate::error::Result;
use crate::protocol::types::{Amount, CommerceCode, OperationNumber, Ticket};
use crate::protocol::{
    FC_CLOSE, FC_LAST_SALE, FC_LOAD_KEYS, FC_MULTICODE_LAST_SALE, FC_MULTICODE_SALE,
    FC_MULTICODE_SALES_DETAIL, FC_NORMAL_MODE, FC_POLL, FC_REFUND, FC_SALE, FC_SALES_DETAIL,
    FC_TOTALS,
};

/// Sale request: validated amount and ticket, and whether to return the voucher.
///
/// ```
/// use transbank_pos::SaleRequest;
/// use transbank_pos::message::sale_payload;
///
/// let req = SaleRequest::new(12_000, "ABC123")?;
/// assert_eq!(sale_payload(&req, false), "0200|000012000|ABC123||0|0");
/// # Ok::<(), transbank_pos::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SaleRequest {
    amount: Amount,
    ticket: Ticket,
    #[cfg_attr(feature = "serde", serde(default))]
    voucher: bool,
}

impl SaleRequest {
    /// Validates the amount (CLP) and the ticket (protocol-spec §4.1).
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`](crate::Error::InvalidArgument) if the amount or the
    /// ticket is invalid; nothing is sent to the POS.
    pub fn new(amount_clp: u32, ticket: &str) -> Result<Self> {
        Ok(Self::from_parts(
            Amount::new(amount_clp)?,
            Ticket::new(ticket)?,
        ))
    }

    /// Builds a request from already validated parts (without voucher).
    #[must_use]
    pub fn from_parts(amount: Amount, ticket: Ticket) -> Self {
        Self {
            amount,
            ticket,
            voucher: false,
        }
    }

    /// Asks the POS to return the voucher in the response.
    #[must_use]
    pub fn with_voucher(mut self, voucher: bool) -> Self {
        self.voucher = voucher;
        self
    }

    /// Amount of the sale.
    #[must_use]
    pub fn amount(&self) -> Amount {
        self.amount
    }

    /// Ticket of the sale.
    #[must_use]
    pub fn ticket(&self) -> &Ticket {
        &self.ticket
    }

    /// Whether the voucher is requested.
    #[must_use]
    pub fn voucher(&self) -> bool {
        self.voucher
    }
}

/// Multi-code sale request: a sale for a child commerce (`"0"` = main commerce).
///
/// ```
/// use transbank_pos::MultiCodeSaleRequest;
/// use transbank_pos::message::multicode_sale_payload;
///
/// let req = MultiCodeSaleRequest::new(5_000, "7", "597012345678")?;
/// assert_eq!(
///     multicode_sale_payload(&req, true),
///     "0270|000005000|000007||0|1|597012345678"
/// );
/// # Ok::<(), transbank_pos::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MultiCodeSaleRequest {
    sale: SaleRequest,
    commerce_code: CommerceCode,
}

impl MultiCodeSaleRequest {
    /// Validates amount, ticket and commerce code (protocol-spec §4.1).
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`](crate::Error::InvalidArgument) if any argument is
    /// invalid; nothing is sent to the POS.
    pub fn new(amount_clp: u32, ticket: &str, commerce_code: &str) -> Result<Self> {
        Ok(Self {
            sale: SaleRequest::new(amount_clp, ticket)?,
            commerce_code: CommerceCode::new(commerce_code)?,
        })
    }

    /// Asks the POS to return the voucher in the response.
    #[must_use]
    pub fn with_voucher(mut self, voucher: bool) -> Self {
        self.sale.voucher = voucher;
        self
    }

    /// Amount, ticket and voucher flag.
    #[must_use]
    pub fn sale(&self) -> &SaleRequest {
        &self.sale
    }

    /// Commerce code of the child commerce.
    #[must_use]
    pub fn commerce_code(&self) -> &CommerceCode {
        &self.commerce_code
    }
}

/// What the POS does with the sales detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SalesDetailMode {
    /// The POS prints the detail; no data is returned (`report = 0`).
    PrintOnPos,
    /// The POS sends one frame per sale to the host (`report = 1`).
    SendToHost,
}

impl SalesDetailMode {
    fn digit(self) -> char {
        match self {
            Self::PrintOnPos => '0',
            Self::SendToHost => '1',
        }
    }
}

fn flag(value: bool) -> char {
    if value { '1' } else { '0' }
}

/// `0100`.
#[must_use]
pub fn poll_payload() -> String {
    FC_POLL.to_owned()
}

/// `0800`.
#[must_use]
pub fn load_keys_payload() -> String {
    FC_LOAD_KEYS.to_owned()
}

/// `0200|{amount:09}|{ticket:06}||{voucher}|{status}`; `status` asks for `0900`
/// intermediate messages.
#[must_use]
pub fn sale_payload(req: &SaleRequest, status: bool) -> String {
    format!(
        "{FC_SALE}|{:09}|{}||{}|{}",
        req.amount.get(),
        req.ticket.padded(),
        flag(req.voucher),
        flag(status)
    )
}

/// `0270|{amount:09}|{ticket:06}||{voucher}|{status}|{commerce_code}`.
#[must_use]
pub fn multicode_sale_payload(req: &MultiCodeSaleRequest, status: bool) -> String {
    format!(
        "{FC_MULTICODE_SALE}|{:09}|{}||{}|{}|{}",
        req.sale.amount.get(),
        req.sale.ticket.padded(),
        flag(req.sale.voucher),
        flag(status),
        req.commerce_code.as_str()
    )
}

/// `0250|`.
#[must_use]
pub fn last_sale_payload() -> String {
    format!("{FC_LAST_SALE}|")
}

/// `0280|{voucher}`.
#[must_use]
pub fn multicode_last_sale_payload(voucher: bool) -> String {
    format!("{FC_MULTICODE_LAST_SALE}|{}", flag(voucher))
}

/// `1200|{operation:06}|`.
#[must_use]
pub fn refund_payload(operation: OperationNumber) -> String {
    format!("{FC_REFUND}|{:06}|", operation.get())
}

/// `0700||`.
#[must_use]
pub fn totals_payload() -> String {
    format!("{FC_TOTALS}||")
}

/// `0260|{0 = print on POS, 1 = send to host}|`.
#[must_use]
pub fn sales_detail_payload(mode: SalesDetailMode) -> String {
    format!("{FC_SALES_DETAIL}|{}|", mode.digit())
}

/// `0290|{0 = print on POS, 1 = send to host}|`.
#[must_use]
pub fn multicode_sales_detail_payload(mode: SalesDetailMode) -> String {
    format!("{FC_MULTICODE_SALES_DETAIL}|{}|", mode.digit())
}

/// `0500||`.
#[must_use]
pub fn close_payload() -> String {
    format!("{FC_CLOSE}||")
}

/// `0300`.
#[must_use]
pub fn normal_mode_payload() -> String {
    FC_NORMAL_MODE.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;

    #[test]
    fn simple_payloads() {
        assert_eq!(poll_payload(), "0100");
        assert_eq!(load_keys_payload(), "0800");
        assert_eq!(last_sale_payload(), "0250|");
        assert_eq!(multicode_last_sale_payload(true), "0280|1");
        assert_eq!(multicode_last_sale_payload(false), "0280|0");
        assert_eq!(totals_payload(), "0700||");
        assert_eq!(close_payload(), "0500||");
        assert_eq!(normal_mode_payload(), "0300");
    }

    #[test]
    fn sale_payloads() {
        let req = SaleRequest::new(12_000, "ABC123").unwrap();
        assert_eq!(sale_payload(&req, false), "0200|000012000|ABC123||0|0");
        let req = SaleRequest::new(50, "42").unwrap().with_voucher(true);
        assert_eq!(sale_payload(&req, true), "0200|000000050|000042||1|1");
        let req = SaleRequest::new(999_999_999, "abc").unwrap();
        assert_eq!(sale_payload(&req, false), "0200|999999999|000abc||0|0");
    }

    #[test]
    fn multicode_payloads() {
        let req = MultiCodeSaleRequest::new(12_000, "ABC123", "0").unwrap();
        assert_eq!(
            multicode_sale_payload(&req, false),
            "0270|000012000|ABC123||0|0|0"
        );
        assert_eq!(
            multicode_sales_detail_payload(SalesDetailMode::SendToHost),
            "0290|1|"
        );
        assert!(matches!(
            MultiCodeSaleRequest::new(12_000, "1", "12a"),
            Err(Error::InvalidArgument {
                field: "commerce_code",
                ..
            })
        ));
    }

    #[test]
    fn refund_and_detail_payloads() {
        assert_eq!(
            refund_payload(OperationNumber::new(142).unwrap()),
            "1200|000142|"
        );
        assert_eq!(sales_detail_payload(SalesDetailMode::SendToHost), "0260|1|");
        assert_eq!(sales_detail_payload(SalesDetailMode::PrintOnPos), "0260|0|");
    }

    #[test]
    fn request_validation() {
        assert!(SaleRequest::new(49, "1").is_err());
        assert!(SaleRequest::new(1_000_000_000, "1").is_err());
        for bad in ["", "1234567", "12-3", "ñ"] {
            assert!(matches!(
                SaleRequest::new(1_000, bad),
                Err(Error::InvalidArgument {
                    field: "ticket",
                    ..
                })
            ));
        }
    }

    #[test]
    fn payloads_encode_as_frames() {
        let req = SaleRequest::new(12_000, "ABC123").unwrap();
        let frame = crate::protocol::encode_frame(&sale_payload(&req, false)).unwrap();
        assert_eq!(frame.last(), Some(&0x3E));
    }
}
