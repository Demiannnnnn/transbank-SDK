//! POS Autoservicio requests, responses and parsers (`docs/protocol-spec.md` §7).

use std::fmt;

use super::integrado::{MultiCodeSaleRequest, SaleRequest};
use super::responses::{Redacted, voucher_lines};
use crate::codes::ResponseCode;
use crate::error::Result;
use crate::protocol::fields::Fields;
use crate::protocol::types::{CardBrand, CardType, PosDate, PosTime};
use crate::protocol::{
    FC_CLOSE, FC_INITIALIZATION_RESPONSE, FC_INITIALIZATION_RESULT, FC_INITIALIZE, FC_LAST_SALE,
    FC_LAST_SALE_RESPONSE, FC_MULTICODE_SALE, FC_MULTICODE_SALE_RESPONSE, FC_REFUND, FC_SALE,
    FC_SALE_RESPONSE,
};

fn flag(value: bool) -> char {
    if value { '1' } else { '0' }
}

/// Initialization request payload (`0070`).
#[must_use]
pub fn initialize_payload() -> String {
    FC_INITIALIZE.to_owned()
}

/// Initialization result request payload (`0080`).
#[must_use]
pub fn initialization_result_payload() -> String {
    FC_INITIALIZATION_RESULT.to_owned()
}

/// Sale request payload (`0200|{amount:09}|{ticket:06}|{voucher}|{status}`).
#[must_use]
pub fn self_service_sale_payload(req: &SaleRequest, status: bool) -> String {
    format!(
        "{FC_SALE}|{:09}|{}|{}|{}",
        req.amount().get(),
        req.ticket().padded(),
        flag(req.voucher()),
        flag(status)
    )
}

/// Multi-code sale request payload (`0270|{amount:09}|{ticket:06}|{voucher}|{status}|{commerce_code}`).
#[must_use]
pub fn self_service_multicode_sale_payload(req: &MultiCodeSaleRequest, status: bool) -> String {
    format!(
        "{FC_MULTICODE_SALE}|{:09}|{}|{}|{}|{}",
        req.sale().amount().get(),
        req.sale().ticket().padded(),
        flag(req.sale().voucher()),
        flag(status),
        req.commerce_code().as_str()
    )
}

/// Last sale request payload (`0250|{b}`).
#[must_use]
pub fn self_service_last_sale_payload(voucher: bool) -> String {
    format!("{FC_LAST_SALE}|{}", flag(voucher))
}

/// Refund request payload (`1200`).
#[must_use]
pub fn self_service_refund_payload() -> String {
    FC_REFUND.to_owned()
}

/// Day close request payload (`0500|{b}`).
#[must_use]
pub fn self_service_close_payload(voucher: bool) -> String {
    format!("{FC_CLOSE}|{}", flag(voucher))
}

/// Sale response (`0210` or `0260`) of the POS Autoservicio (`docs/protocol-spec.md` §7.1).
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct SelfServiceSaleResponse {
    /// Function code (`0210`, `0260`, or `0271`).
    pub function_code: String,
    /// Response code; see [`SelfServiceSaleResponse::is_approved`].
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
    /// Last 4 digits of the card (leading zeros kept).
    pub last4_digits: Option<String>,
    /// Operation number (leading zeros kept).
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
    /// Voucher text (only if requested). Redacted in `Debug`.
    pub voucher: Option<String>,
    /// Installments type code.
    pub installments_type: Option<String>,
    /// Number of installments.
    pub installments_number: Option<u32>,
    /// Amount of each installment in CLP.
    pub installments_amount: Option<u32>,
    /// Installments type description.
    pub installments_type_description: Option<String>,
    /// Raw payload.
    pub raw: String,
}

impl SelfServiceSaleResponse {
    /// `true` only if the sale was approved (`response_code == ResponseCode::Approved`).
    #[must_use]
    pub fn is_approved(&self) -> bool {
        self.response_code == ResponseCode::Approved
    }

    /// Voucher split in lines of 40 characters, if its length is a multiple of 40.
    #[must_use]
    pub fn voucher_lines(&self) -> Option<Vec<&str>> {
        voucher_lines(self.voucher.as_ref())
    }

    fn from_fields(fields: &Fields<'_>) -> Result<Self> {
        Ok(Self {
            function_code: fields.function_code().to_owned(),
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
            voucher: fields.raw_at(15),
            installments_type: fields.str_at(16),
            installments_number: fields.u32_at(17, "installments_number")?,
            installments_amount: fields.u32_at(18, "installments_amount")?,
            installments_type_description: fields.str_at(19),
            raw: fields.payload().to_owned(),
        })
    }
}

impl fmt::Debug for SelfServiceSaleResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SelfServiceSaleResponse")
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
            .field("voucher", &Redacted(&self.voucher))
            .field("installments_type", &self.installments_type)
            .field("installments_number", &self.installments_number)
            .field("installments_amount", &self.installments_amount)
            .field(
                "installments_type_description",
                &self.installments_type_description,
            )
            .field(
                "raw",
                &format_args!("<redacted {} chars>", self.raw.chars().count()),
            )
            .finish()
    }
}

/// Multi-code sale response (`0271`) of the POS Autoservicio.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct SelfServiceMultiCodeSaleResponse {
    /// Fields of the sale.
    pub sale: SelfServiceSaleResponse,
    /// Code of the child commerce (field 20).
    pub commerce_provider_code: Option<String>,
}

impl SelfServiceMultiCodeSaleResponse {
    /// `true` only if the sale was approved.
    #[must_use]
    pub fn is_approved(&self) -> bool {
        self.sale.is_approved()
    }
}

/// Initialization response (`1080`) of the POS Autoservicio.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct InitializationResponse {
    /// Response code; `90` ([`ResponseCode::InitializationOk`]) indicates success.
    pub response_code: ResponseCode,
    /// Date configured in the POS.
    pub date: Option<PosDate>,
    /// Time configured in the POS.
    pub time: Option<PosTime>,
    /// Raw payload.
    pub raw: String,
}

impl InitializationResponse {
    /// `true` only if initialization succeeded (`response_code == ResponseCode::InitializationOk`).
    #[must_use]
    pub fn is_approved(&self) -> bool {
        self.response_code == ResponseCode::InitializationOk
    }
}

/// Parses a POS Autoservicio sale (`0210`) or last sale (`0260`) response.
///
/// # Errors
///
/// [`Error::UnexpectedResponse`](crate::Error::UnexpectedResponse) for another function code;
/// [`Error::Parse`](crate::Error::Parse) if the response code is missing or a numeric field is invalid.
pub fn parse_self_service_sale(payload: &str) -> Result<SelfServiceSaleResponse> {
    let fields = Fields::new(payload);
    fields.expect_function(&[FC_SALE_RESPONSE, FC_LAST_SALE_RESPONSE])?;
    SelfServiceSaleResponse::from_fields(&fields)
}

/// Parses a POS Autoservicio multi-code sale response (`0271`).
///
/// # Errors
///
/// Same as [`parse_self_service_sale`].
pub fn parse_self_service_multicode_sale(
    payload: &str,
) -> Result<SelfServiceMultiCodeSaleResponse> {
    let fields = Fields::new(payload);
    fields.expect_function(&[FC_MULTICODE_SALE_RESPONSE])?;
    Ok(SelfServiceMultiCodeSaleResponse {
        sale: SelfServiceSaleResponse::from_fields(&fields)?,
        commerce_provider_code: fields.str_at(20),
    })
}

/// Parses a POS Autoservicio initialization response (`1080`).
///
/// # Errors
///
/// Same as [`parse_self_service_sale`].
pub fn parse_initialization(payload: &str) -> Result<InitializationResponse> {
    let fields = Fields::new(payload);
    fields.expect_function(&[FC_INITIALIZATION_RESPONSE])?;
    Ok(InitializationResponse {
        response_code: fields.required_code()?,
        date: fields.date_at(2),
        time: fields.time_at(3),
        raw: payload.to_owned(),
    })
}
