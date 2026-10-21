//! POS Integrado responses and their parsers (`docs/protocol-spec.md` §5, §6).
//!
//! Every parser checks the function code ([`Error::UnexpectedResponse`] otherwise),
//! keeps the raw payload in `raw` and never invents values for absent fields
//! (PROTO-05, PROTO-06). A declined sale parses as `Ok` with a non-approved
//! [`ResponseCode`] (ADR-03).

use std::fmt;

use crate::codes::{IntermediateStatus, ResponseCode};
use crate::error::Result;
use crate::protocol::fields::Fields;
use crate::protocol::types::{CardBrand, CardType, PosDate, PosTime, parse_digits};
use crate::protocol::{
    FC_CLOSE_RESPONSE, FC_INTERMEDIATE, FC_LAST_SALE_RESPONSE, FC_LOAD_KEYS_RESPONSE,
    FC_MULTICODE_LAST_SALE_RESPONSE, FC_MULTICODE_SALE_RESPONSE,
    FC_MULTICODE_SALES_DETAIL_RESPONSE, FC_REFUND_RESPONSE, FC_SALE_RESPONSE,
    FC_SALES_DETAIL_RESPONSE, FC_TOTALS_RESPONSE,
};

/// Width of a voucher line.
const VOUCHER_LINE: usize = 40;

/// Shows the length of a sensitive value, never its content (SEC-03).
struct Redacted<'a>(&'a Option<String>);

impl fmt::Debug for Redacted<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(text) => write!(f, "Some(<redacted {} chars>)", text.chars().count()),
            None => f.write_str("None"),
        }
    }
}

fn voucher_lines(voucher: Option<&String>) -> Option<Vec<&str>> {
    let voucher = voucher?;
    let count = voucher.chars().count();
    if count == 0 || count % VOUCHER_LINE != 0 {
        return None;
    }
    let mut starts: Vec<usize> = voucher
        .char_indices()
        .step_by(VOUCHER_LINE)
        .map(|(index, _)| index)
        .collect();
    starts.push(voucher.len());
    Some(
        starts
            .windows(2)
            .filter_map(|pair| match pair {
                [start, end] => voucher.get(*start..*end),
                _ => None,
            })
            .collect(),
    )
}

/// Sale (`0210`) or last sale (`0260`) of the POS Integrado.
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct SaleResponse {
    /// Function code (`0210`, `0260`, or the multi-code ones).
    pub function_code: String,
    /// Response code; see [`SaleResponse::is_approved`].
    pub response_code: ResponseCode,
    /// Commerce code.
    pub commerce_code: Option<String>,
    /// Terminal id.
    pub terminal_id: Option<String>,
    /// Ticket, as reported (leading zeros kept).
    pub ticket: Option<String>,
    /// Authorization code.
    pub authorization_code: Option<String>,
    /// Amount in CLP.
    pub amount: Option<u32>,
    /// Number of installments.
    pub installments_number: Option<u32>,
    /// Amount of each installment in CLP.
    pub installments_amount: Option<u32>,
    /// Last 4 digits of the card (leading zeros kept).
    pub last4_digits: Option<String>,
    /// Operation number, used to refund the sale (leading zeros kept).
    pub operation_number: Option<String>,
    /// Card type.
    pub card_type: Option<CardType>,
    /// Accounting date.
    pub accounting_date: Option<PosDate>,
    /// Account number, masked by the POS. Redacted in `Debug`.
    pub account_number: Option<String>,
    /// Card brand.
    pub card_brand: Option<CardBrand>,
    /// Date of the transaction.
    pub real_date: Option<PosDate>,
    /// Time of the transaction.
    pub real_time: Option<PosTime>,
    /// Employee id.
    pub employee_id: Option<u32>,
    /// Tip in CLP.
    pub tip: Option<u32>,
    /// Voucher text (only if requested). Redacted in `Debug`.
    pub voucher: Option<String>,
    /// Raw payload.
    pub raw: String,
}

impl SaleResponse {
    /// `true` only if the POS approved the sale.
    #[must_use]
    pub fn is_approved(&self) -> bool {
        self.response_code.is_approved()
    }

    /// Voucher split in lines of 40 characters, if its length is a multiple of 40.
    #[must_use]
    pub fn voucher_lines(&self) -> Option<Vec<&str>> {
        voucher_lines(self.voucher.as_ref())
    }

    /// Fields 0–18 of the `0210` layout; the voucher comes from `voucher_index`.
    fn from_fields(fields: &Fields<'_>, voucher_index: Option<usize>) -> Result<Self> {
        Ok(Self {
            function_code: fields.function_code().to_owned(),
            response_code: fields.required_code()?,
            commerce_code: fields.str_at(2),
            terminal_id: fields.str_at(3),
            ticket: fields.str_at(4),
            authorization_code: fields.str_at(5),
            amount: fields.u32_at(6, "amount")?,
            installments_number: fields.u32_at(7, "installments_number")?,
            installments_amount: fields.u32_at(8, "installments_amount")?,
            last4_digits: fields.str_at(9),
            operation_number: fields.str_at(10),
            card_type: fields.card_type_at(11),
            accounting_date: fields.date_at(12),
            account_number: fields.str_at(13),
            card_brand: fields.card_brand_at(14),
            real_date: fields.date_at(15),
            real_time: fields.time_at(16),
            employee_id: fields.u32_at(17, "employee_id")?,
            tip: fields.u32_at(18, "tip")?,
            voucher: voucher_index.and_then(|index| fields.raw_at(index)),
            raw: fields.payload().to_owned(),
        })
    }
}

impl fmt::Debug for SaleResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SaleResponse")
            .field("function_code", &self.function_code)
            .field("response_code", &self.response_code)
            .field("commerce_code", &self.commerce_code)
            .field("terminal_id", &self.terminal_id)
            .field("ticket", &self.ticket)
            .field("authorization_code", &self.authorization_code)
            .field("amount", &self.amount)
            .field("installments_number", &self.installments_number)
            .field("installments_amount", &self.installments_amount)
            .field("last4_digits", &self.last4_digits)
            .field("operation_number", &self.operation_number)
            .field("card_type", &self.card_type)
            .field("accounting_date", &self.accounting_date)
            .field("account_number", &Redacted(&self.account_number))
            .field("card_brand", &self.card_brand)
            .field("real_date", &self.real_date)
            .field("real_time", &self.real_time)
            .field("employee_id", &self.employee_id)
            .field("tip", &self.tip)
            .field("voucher", &Redacted(&self.voucher))
            .field(
                "raw",
                &format_args!("<redacted {} chars>", self.raw.chars().count()),
            )
            .finish()
    }
}

/// Multi-code sale (`0271`) or multi-code last sale (`0281`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct MultiCodeSaleResponse {
    /// Fields of the sale (field 19 is the voucher, if any).
    pub sale: SaleResponse,
    /// Change in CLP (field 20).
    pub change: Option<u32>,
    /// Code of the child commerce (field 21).
    pub commerce_provider_code: Option<String>,
}

impl MultiCodeSaleResponse {
    /// `true` only if the POS approved the sale.
    #[must_use]
    pub fn is_approved(&self) -> bool {
        self.sale.is_approved()
    }
}

/// One entry of the sales detail (`0261`).
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct DetailEntry {
    /// Function code (`0261`).
    pub function_code: String,
    /// Response code.
    pub response_code: ResponseCode,
    /// Commerce code.
    pub commerce_code: Option<String>,
    /// Terminal id.
    pub terminal_id: Option<String>,
    /// Ticket (leading zeros kept).
    pub ticket: Option<String>,
    /// Authorization code; empty marks the end of the list.
    pub authorization_code: Option<String>,
    /// Amount in CLP.
    pub amount: Option<u32>,
    /// Last 4 digits of the card.
    pub last4_digits: Option<String>,
    /// Operation number.
    pub operation_number: Option<String>,
    /// Card type.
    pub card_type: Option<CardType>,
    /// Accounting date.
    pub accounting_date: Option<PosDate>,
    /// Account number, masked by the POS. Redacted in `Debug`.
    pub account_number: Option<String>,
    /// Card brand.
    pub card_brand: Option<CardBrand>,
    /// Date of the transaction.
    pub real_date: Option<PosDate>,
    /// Time of the transaction.
    pub real_time: Option<PosTime>,
    /// Employee id.
    pub employee_id: Option<u32>,
    /// Tip in CLP.
    pub tip: Option<u32>,
    /// Amount of each installment in CLP.
    pub installments_amount: Option<u32>,
    /// Number of installments.
    pub installments_number: Option<u32>,
    /// Raw payload.
    pub raw: String,
}

impl DetailEntry {
    /// `true` only if the entry is an approved sale.
    #[must_use]
    pub fn is_approved(&self) -> bool {
        self.response_code.is_approved()
    }
}

impl fmt::Debug for DetailEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DetailEntry")
            .field("function_code", &self.function_code)
            .field("response_code", &self.response_code)
            .field("commerce_code", &self.commerce_code)
            .field("terminal_id", &self.terminal_id)
            .field("ticket", &self.ticket)
            .field("authorization_code", &self.authorization_code)
            .field("amount", &self.amount)
            .field("last4_digits", &self.last4_digits)
            .field("operation_number", &self.operation_number)
            .field("card_type", &self.card_type)
            .field("accounting_date", &self.accounting_date)
            .field("account_number", &Redacted(&self.account_number))
            .field("card_brand", &self.card_brand)
            .field("real_date", &self.real_date)
            .field("real_time", &self.real_time)
            .field("employee_id", &self.employee_id)
            .field("tip", &self.tip)
            .field("installments_amount", &self.installments_amount)
            .field("installments_number", &self.installments_number)
            .field(
                "raw",
                &format_args!("<redacted {} chars>", self.raw.chars().count()),
            )
            .finish()
    }
}

/// One entry of the multi-code sales detail (`0291`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct MultiCodeDetailEntry {
    /// Fields 0–18 with the `0210` layout (no voucher).
    pub sale: SaleResponse,
    /// Change in CLP (field 19).
    pub change: Option<u32>,
    /// Code of the child commerce (field 20).
    pub commerce_provider_code: Option<String>,
}

impl MultiCodeDetailEntry {
    /// `true` only if the entry is an approved sale.
    #[must_use]
    pub fn is_approved(&self) -> bool {
        self.sale.is_approved()
    }
}

/// Load keys (`0810`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct LoadKeysResponse {
    /// Response code.
    pub response_code: ResponseCode,
    /// Commerce code.
    pub commerce_code: Option<String>,
    /// Terminal id.
    pub terminal_id: Option<String>,
    /// Raw payload.
    pub raw: String,
}

impl LoadKeysResponse {
    /// `true` if the keys were loaded.
    #[must_use]
    pub fn is_approved(&self) -> bool {
        self.response_code.is_approved()
    }
}

/// Day close (`0510`).
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct CloseResponse {
    /// Response code.
    pub response_code: ResponseCode,
    /// Commerce code.
    pub commerce_code: Option<String>,
    /// Terminal id.
    pub terminal_id: Option<String>,
    /// Voucher of the close, if sent. Redacted in `Debug`.
    pub voucher: Option<String>,
    /// Raw payload.
    pub raw: String,
}

impl CloseResponse {
    /// `true` if the close succeeded.
    #[must_use]
    pub fn is_approved(&self) -> bool {
        self.response_code.is_approved()
    }

    /// Voucher split in lines of 40 characters, if its length is a multiple of 40.
    #[must_use]
    pub fn voucher_lines(&self) -> Option<Vec<&str>> {
        voucher_lines(self.voucher.as_ref())
    }
}

impl fmt::Debug for CloseResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CloseResponse")
            .field("response_code", &self.response_code)
            .field("commerce_code", &self.commerce_code)
            .field("terminal_id", &self.terminal_id)
            .field("voucher", &Redacted(&self.voucher))
            .field(
                "raw",
                &format_args!("<redacted {} chars>", self.raw.chars().count()),
            )
            .finish()
    }
}

/// Totals since the last close (`0710`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct TotalsResponse {
    /// Response code.
    pub response_code: ResponseCode,
    /// Number of transactions.
    pub tx_count: Option<u32>,
    /// Total amount in CLP.
    pub tx_total: Option<u64>,
    /// Raw payload.
    pub raw: String,
}

impl TotalsResponse {
    /// `true` if the POS answered successfully.
    #[must_use]
    pub fn is_approved(&self) -> bool {
        self.response_code.is_approved()
    }
}

/// Refund (`1210`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct RefundResponse {
    /// Response code.
    pub response_code: ResponseCode,
    /// Commerce code.
    pub commerce_code: Option<String>,
    /// Terminal id.
    pub terminal_id: Option<String>,
    /// Authorization code of the refund.
    pub authorization_code: Option<String>,
    /// Operation number of the refund (leading zeros kept).
    pub operation_number: Option<String>,
    /// Raw payload.
    pub raw: String,
}

impl RefundResponse {
    /// `true` if the refund was approved.
    #[must_use]
    pub fn is_approved(&self) -> bool {
        self.response_code.is_approved()
    }
}

/// Parses a sale (`0210`) or last sale (`0260`) response.
///
/// # Errors
///
/// [`Error::UnexpectedResponse`](crate::Error::UnexpectedResponse) for another
/// function code; [`Error::Parse`](crate::Error::Parse) if the response code is
/// missing or a numeric field is invalid.
///
/// ```
/// use transbank_pos::ResponseCode;
/// use transbank_pos::message::parse_sale;
///
/// let sale = parse_sale("0210|00|597029414300|IT750050|abc123|757752|12000|03|3334|9480|000135|CR|22102025|0000000000000000000|MC|22102025|114016|7|500|")?;
/// assert!(sale.is_approved());
/// assert_eq!(sale.amount, Some(12_000));
/// assert_eq!(sale.last4_digits.as_deref(), Some("9480"));
///
/// let declined = parse_sale("0210|01|597029414300|IT750050")?;
/// assert_eq!(declined.response_code, ResponseCode::Rejected);
/// # Ok::<(), transbank_pos::Error>(())
/// ```
pub fn parse_sale(payload: &str) -> Result<SaleResponse> {
    let fields = Fields::new(payload);
    fields.expect_function(&[FC_SALE_RESPONSE, FC_LAST_SALE_RESPONSE])?;
    SaleResponse::from_fields(&fields, Some(19))
}

/// Parses a multi-code sale (`0271`) or multi-code last sale (`0281`) response.
///
/// # Errors
///
/// Same as [`parse_sale`].
pub fn parse_multicode_sale(payload: &str) -> Result<MultiCodeSaleResponse> {
    let fields = Fields::new(payload);
    fields.expect_function(&[FC_MULTICODE_SALE_RESPONSE, FC_MULTICODE_LAST_SALE_RESPONSE])?;
    Ok(MultiCodeSaleResponse {
        sale: SaleResponse::from_fields(&fields, Some(19))?,
        change: fields.u32_at(20, "change")?,
        commerce_provider_code: fields.str_at(21),
    })
}

/// Parses one sales detail entry (`0261`).
///
/// # Errors
///
/// Same as [`parse_sale`].
pub fn parse_detail_entry(payload: &str) -> Result<DetailEntry> {
    let fields = Fields::new(payload);
    let function_code = fields.expect_function(&[FC_SALES_DETAIL_RESPONSE])?;
    Ok(DetailEntry {
        function_code: function_code.to_owned(),
        response_code: fields.required_code()?,
        commerce_code: fields.str_at(2),
        terminal_id: fields.str_at(3),
        ticket: fields.str_at(4),
        authorization_code: fields.str_at(5),
        amount: fields.u32_at(6, "amount")?,
        last4_digits: fields.str_at(7),
        operation_number: fields.str_at(8),
        card_type: fields.card_type_at(9),
        accounting_date: fields.date_at(10),
        account_number: fields.str_at(11),
        card_brand: fields.card_brand_at(12),
        real_date: fields.date_at(13),
        real_time: fields.time_at(14),
        employee_id: fields.u32_at(15, "employee_id")?,
        tip: fields.u32_at(16, "tip")?,
        installments_amount: fields.u32_at(17, "installments_amount")?,
        installments_number: fields.u32_at(18, "installments_number")?,
        raw: payload.to_owned(),
    })
}

/// Parses one multi-code sales detail entry (`0291`).
///
/// # Errors
///
/// Same as [`parse_sale`].
pub fn parse_multicode_detail_entry(payload: &str) -> Result<MultiCodeDetailEntry> {
    let fields = Fields::new(payload);
    fields.expect_function(&[FC_MULTICODE_SALES_DETAIL_RESPONSE])?;
    Ok(MultiCodeDetailEntry {
        sale: SaleResponse::from_fields(&fields, None)?,
        change: fields.u32_at(19, "change")?,
        commerce_provider_code: fields.str_at(20),
    })
}

/// `true` if a detail entry marks the end of the list (empty authorization code,
/// `docs/protocol-spec.md` §4.2).
#[must_use]
pub fn is_detail_terminator(entry: &DetailEntry) -> bool {
    entry.authorization_code.is_none()
}

/// Parses a load keys response (`0810`).
///
/// # Errors
///
/// Same as [`parse_sale`].
pub fn parse_load_keys(payload: &str) -> Result<LoadKeysResponse> {
    let fields = Fields::new(payload);
    fields.expect_function(&[FC_LOAD_KEYS_RESPONSE])?;
    Ok(LoadKeysResponse {
        response_code: fields.required_code()?,
        commerce_code: fields.str_at(2),
        terminal_id: fields.str_at(3),
        raw: payload.to_owned(),
    })
}

/// Parses a close response (`0510`).
///
/// # Errors
///
/// Same as [`parse_sale`].
pub fn parse_close(payload: &str) -> Result<CloseResponse> {
    let fields = Fields::new(payload);
    fields.expect_function(&[FC_CLOSE_RESPONSE])?;
    Ok(CloseResponse {
        response_code: fields.required_code()?,
        commerce_code: fields.str_at(2),
        terminal_id: fields.str_at(3),
        voucher: fields.raw_at(4),
        raw: payload.to_owned(),
    })
}

/// Parses a totals response (`0710`).
///
/// # Errors
///
/// Same as [`parse_sale`].
pub fn parse_totals(payload: &str) -> Result<TotalsResponse> {
    let fields = Fields::new(payload);
    fields.expect_function(&[FC_TOTALS_RESPONSE])?;
    Ok(TotalsResponse {
        response_code: fields.required_code()?,
        tx_count: fields.u32_at(2, "tx_count")?,
        tx_total: fields.u64_at(3, "tx_total")?,
        raw: payload.to_owned(),
    })
}

/// Parses a refund response (`1210`).
///
/// # Errors
///
/// Same as [`parse_sale`].
pub fn parse_refund(payload: &str) -> Result<RefundResponse> {
    let fields = Fields::new(payload);
    fields.expect_function(&[FC_REFUND_RESPONSE])?;
    Ok(RefundResponse {
        response_code: fields.required_code()?,
        commerce_code: fields.str_at(2),
        terminal_id: fields.str_at(3),
        authorization_code: fields.str_at(4),
        operation_number: fields.str_at(5),
        raw: payload.to_owned(),
    })
}

/// Parses an intermediate status message (`0900|{code}`).
///
/// A code outside the specification table, or a non-numeric one, is returned as
/// [`IntermediateStatus::Unknown`], never as an error.
///
/// # Errors
///
/// [`Error::UnexpectedResponse`](crate::Error::UnexpectedResponse) for another
/// function code.
pub fn parse_intermediate(payload: &str) -> Result<IntermediateStatus> {
    let fields = Fields::new(payload);
    fields.expect_function(&[FC_INTERMEDIATE])?;
    let text = fields.str_at(1).unwrap_or_default();
    Ok(
        match parse_digits::<u16>(&text).map(ResponseCode::from_code) {
            Some(ResponseCode::Other(_)) | None => IntermediateStatus::Unknown(text),
            Some(code) => IntermediateStatus::Code(code),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;

    const SALE: &str = "0210|00|597029414300|IT750050|abc123|757752|12000|03|3334|9480|000135|CR|22102025|0000000000000000000|MC|22102025|114016|7|500|";

    #[test]
    fn sale_all_fields() {
        let sale = parse_sale(SALE).unwrap();
        assert_eq!(sale.function_code, "0210");
        assert_eq!(sale.response_code, ResponseCode::Approved);
        assert_eq!(sale.commerce_code.as_deref(), Some("597029414300"));
        assert_eq!(sale.terminal_id.as_deref(), Some("IT750050"));
        assert_eq!(sale.ticket.as_deref(), Some("abc123"));
        assert_eq!(sale.authorization_code.as_deref(), Some("757752"));
        assert_eq!(sale.amount, Some(12_000));
        assert_eq!(sale.installments_number, Some(3));
        assert_eq!(sale.installments_amount, Some(3_334));
        assert_eq!(sale.last4_digits.as_deref(), Some("9480"));
        assert_eq!(sale.operation_number.as_deref(), Some("000135"));
        assert_eq!(sale.card_type, Some(CardType::Credit));
        assert_eq!(sale.accounting_date, PosDate::parse("22102025"));
        assert_eq!(sale.account_number.as_deref(), Some("0000000000000000000"));
        assert_eq!(sale.card_brand, Some(CardBrand::Mastercard));
        assert_eq!(sale.real_date, PosDate::parse("22102025"));
        assert_eq!(sale.real_time, PosTime::parse("114016"));
        assert_eq!(sale.employee_id, Some(7));
        assert_eq!(sale.tip, Some(500));
        assert_eq!(sale.voucher, None);
        assert_eq!(sale.raw, SALE);
        assert!(sale.is_approved());
    }

    #[test]
    fn last_sale_uses_same_layout() {
        let payload = SALE.replacen("0210", "0260", 1);
        let sale = parse_sale(&payload).unwrap();
        assert_eq!(sale.function_code, "0260");
        assert_eq!(sale.amount, Some(12_000));
    }

    #[test]
    fn rejected_multicode_sale_is_ok() {
        let sale = parse_multicode_sale("0271|07|||ABC123||90000|").unwrap();
        assert!(!sale.is_approved());
        assert_eq!(sale.sale.response_code, ResponseCode::CanceledAtPos);
        assert_eq!(sale.sale.ticket.as_deref(), Some("ABC123"));
        assert_eq!(sale.sale.amount, Some(90_000));
        assert_eq!(sale.sale.commerce_code, None);
        assert_eq!(sale.sale.authorization_code, None);
        assert_eq!(sale.change, None);
    }

    #[test]
    fn multicode_sale_extra_fields() {
        let payload = format!(
            "{}|0|1500|597012345678",
            SALE.replacen("0210", "0271", 1).trim_end_matches('|')
        );
        let sale = parse_multicode_sale(&payload).unwrap();
        assert_eq!(sale.sale.tip, Some(500));
        assert_eq!(sale.sale.voucher.as_deref(), Some("0"));
        assert_eq!(sale.change, Some(1_500));
        assert_eq!(sale.commerce_provider_code.as_deref(), Some("597012345678"));
    }

    #[test]
    fn detail_entry_all_fields() {
        let entry = parse_detail_entry("0261|00|597029414300|IT750050|abc123|757752|12000|9480|000135|CR|22102025|0000000000000000000|MC|22102025|114016|7|500|3334|03").unwrap();
        assert_eq!(entry.function_code, "0261");
        assert_eq!(entry.response_code, ResponseCode::Approved);
        assert_eq!(entry.commerce_code.as_deref(), Some("597029414300"));
        assert_eq!(entry.terminal_id.as_deref(), Some("IT750050"));
        assert_eq!(entry.ticket.as_deref(), Some("abc123"));
        assert_eq!(entry.authorization_code.as_deref(), Some("757752"));
        assert_eq!(entry.amount, Some(12_000));
        assert_eq!(entry.last4_digits.as_deref(), Some("9480"));
        assert_eq!(entry.operation_number.as_deref(), Some("000135"));
        assert_eq!(entry.card_type, Some(CardType::Credit));
        assert_eq!(entry.accounting_date, PosDate::parse("22102025"));
        assert_eq!(entry.account_number.as_deref(), Some("0000000000000000000"));
        assert_eq!(entry.card_brand, Some(CardBrand::Mastercard));
        assert_eq!(entry.real_date, PosDate::parse("22102025"));
        assert_eq!(entry.real_time, PosTime::parse("114016"));
        assert_eq!(entry.employee_id, Some(7));
        assert_eq!(entry.tip, Some(500));
        assert_eq!(entry.installments_amount, Some(3_334));
        assert_eq!(entry.installments_number, Some(3));
        assert!(!is_detail_terminator(&entry));
    }

    #[test]
    fn detail_terminators() {
        for payload in [
            "0261|00|597029414300|IT750050||||||||||||||||",
            "0261|11||||||||||||||||||",
        ] {
            let entry = parse_detail_entry(payload).unwrap();
            assert!(is_detail_terminator(&entry), "{payload}");
        }
        let no_sales = parse_detail_entry("0261|11||||||||||||||||||").unwrap();
        assert_eq!(no_sales.response_code, ResponseCode::NoSale);
    }

    #[test]
    fn multicode_detail_entry_all_fields() {
        let entry = parse_multicode_detail_entry("0291|00|597029414300|IT750050|abc123|757752|12000|03|3334|9480|000135|CR|22102025|0000000000000000000|MC|22102025|114016|7|500|0|597012345678").unwrap();
        let sale = &entry.sale;
        assert_eq!(sale.function_code, "0291");
        assert_eq!(sale.response_code, ResponseCode::Approved);
        assert_eq!(sale.commerce_code.as_deref(), Some("597029414300"));
        assert_eq!(sale.terminal_id.as_deref(), Some("IT750050"));
        assert_eq!(sale.ticket.as_deref(), Some("abc123"));
        assert_eq!(sale.authorization_code.as_deref(), Some("757752"));
        assert_eq!(sale.amount, Some(12_000));
        assert_eq!(sale.installments_number, Some(3));
        assert_eq!(sale.installments_amount, Some(3_334));
        assert_eq!(sale.last4_digits.as_deref(), Some("9480"));
        assert_eq!(sale.operation_number.as_deref(), Some("000135"));
        assert_eq!(sale.card_type, Some(CardType::Credit));
        assert_eq!(sale.accounting_date, PosDate::parse("22102025"));
        assert_eq!(sale.account_number.as_deref(), Some("0000000000000000000"));
        assert_eq!(sale.card_brand, Some(CardBrand::Mastercard));
        assert_eq!(sale.real_date, PosDate::parse("22102025"));
        assert_eq!(sale.real_time, PosTime::parse("114016"));
        assert_eq!(sale.employee_id, Some(7));
        assert_eq!(sale.tip, Some(500));
        assert_eq!(sale.voucher, None);
        assert_eq!(entry.change, Some(0));
        assert_eq!(
            entry.commerce_provider_code.as_deref(),
            Some("597012345678")
        );
    }

    #[test]
    fn totals_all_fields() {
        let totals = parse_totals("0710|00|002|15000||").unwrap();
        assert_eq!(totals.response_code, ResponseCode::Approved);
        assert_eq!(totals.tx_count, Some(2));
        assert_eq!(totals.tx_total, Some(15_000));
        assert_eq!(totals.raw, "0710|00|002|15000||");
    }

    #[test]
    fn refund_all_fields() {
        let refund = parse_refund("1210|00|597029414300|IT750050|162529|000142||").unwrap();
        assert!(refund.is_approved());
        assert_eq!(refund.commerce_code.as_deref(), Some("597029414300"));
        assert_eq!(refund.terminal_id.as_deref(), Some("IT750050"));
        assert_eq!(refund.authorization_code.as_deref(), Some("162529"));
        assert_eq!(refund.operation_number.as_deref(), Some("000142"));
    }

    #[test]
    fn load_keys_and_close() {
        let keys = parse_load_keys("0810|00|597029414300|IT750050").unwrap();
        assert!(keys.is_approved());
        assert_eq!(keys.terminal_id.as_deref(), Some("IT750050"));
        let close = parse_close("0510|00|597029414300|IT750050").unwrap();
        assert!(close.is_approved());
        assert_eq!(close.voucher, None);
    }

    #[test]
    fn intermediate_status() {
        assert_eq!(
            parse_intermediate("0900|81").unwrap(),
            IntermediateStatus::Code(ResponseCode::RequestingPin)
        );
        assert_eq!(
            parse_intermediate("0900|99").unwrap(),
            IntermediateStatus::Unknown("99".into())
        );
        assert_eq!(
            parse_intermediate("0900|xx").unwrap(),
            IntermediateStatus::Unknown("xx".into())
        );
        assert!(parse_intermediate("0210|81").is_err());
    }

    #[test]
    fn invalid_number_is_a_parse_error() {
        let payload = SALE.replace("|12000|", "|12a00|");
        assert!(matches!(
            parse_sale(&payload),
            Err(Error::Parse { field: "amount", ref value, .. }) if value == "12a00"
        ));
    }

    #[test]
    fn invalid_date_is_none_and_zeros_are_kept() {
        let payload = SALE
            .replacen("|22102025|", "|00-00-00|", 1)
            .replace("|9480|", "|0331|");
        let sale = parse_sale(&payload).unwrap();
        assert_eq!(sale.accounting_date, None);
        assert_eq!(sale.real_date, PosDate::parse("22102025"));
        assert_eq!(sale.last4_digits.as_deref(), Some("0331"));
        assert!(sale.raw.contains("00-00-00"));
    }

    #[test]
    fn wrong_function_code() {
        assert!(matches!(
            parse_sale("0810|00"),
            Err(Error::UnexpectedResponse { expected: "0210", ref received, .. }) if received == "0810"
        ));
        assert!(matches!(
            parse_totals(""),
            Err(Error::UnexpectedResponse { .. })
        ));
        assert!(matches!(
            parse_refund("0710|00"),
            Err(Error::UnexpectedResponse { .. })
        ));
    }

    #[test]
    fn voucher_lines_and_debug_redaction() {
        let line = "VOUCHER LINE WITH ACCOUNT 1234 ÑANDÚ    ";
        assert_eq!(line.chars().count(), 40);
        let voucher = format!("{line}{line}");
        let payload = format!(
            "{}{voucher}",
            SALE.replace("0000000000000000000", "SECRETACCOUNT")
        );
        let sale = parse_sale(&payload).unwrap();
        assert_eq!(sale.voucher.as_deref(), Some(voucher.as_str()));
        assert_eq!(sale.voucher_lines(), Some(vec![line, line]));

        let debug = format!("{sale:?}");
        assert!(!debug.contains("SECRETACCOUNT"), "{debug}");
        assert!(!debug.contains("VOUCHER"), "{debug}");
        assert!(debug.contains("<redacted 13 chars>"), "{debug}");
        assert!(debug.contains("<redacted 80 chars>"), "{debug}");

        let odd = parse_sale(&format!("{SALE}SHORT")).unwrap();
        assert_eq!(odd.voucher.as_deref(), Some("SHORT"));
        assert_eq!(odd.voucher_lines(), None);
    }

    #[test]
    fn close_and_detail_debug_redact() {
        let close = parse_close("0510|00|597029414300|IT750050|CLOSE VOUCHER TEXT").unwrap();
        assert!(!format!("{close:?}").contains("CLOSE VOUCHER"));
        let entry =
            parse_detail_entry("0261|00|1|T|1|2|3|4|5|CR|22102025|SECRETACCOUNT|MC").unwrap();
        assert!(!format!("{entry:?}").contains("SECRETACCOUNT"));
    }
}
