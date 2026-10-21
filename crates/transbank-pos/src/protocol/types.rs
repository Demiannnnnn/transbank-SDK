//! Validated domain types (API-04) and the date/time/card types reported by the POS.

use std::fmt;
use std::str::FromStr;

use crate::error::{Error, Result};

/// Parses a non-empty string of ASCII digits (no sign, no spaces).
pub(crate) fn parse_digits<T: FromStr>(text: &str) -> Option<T> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// Amount in Chilean pesos (CLP). A sale accepts `50..=999_999_999`.
///
/// ```
/// use transbank_pos::Amount;
///
/// let amount = Amount::new(12_000)?;
/// assert_eq!(amount.get(), 12_000);
/// assert_eq!(amount.to_string(), "$12.000");
/// assert!(Amount::new(49).is_err());
/// # Ok::<(), transbank_pos::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "u32", into = "u32")
)]
pub struct Amount(u32);

impl Amount {
    /// Minimum amount of a sale.
    pub const MIN_SALE: u32 = 50;
    /// Maximum amount (9 digits).
    pub const MAX: u32 = 999_999_999;

    /// Validates a sale amount.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] if `clp` is outside `50..=999_999_999`.
    pub fn new(clp: u32) -> Result<Self> {
        if (Self::MIN_SALE..=Self::MAX).contains(&clp) {
            Ok(Self(clp))
        } else {
            Err(Error::InvalidArgument {
                field: "amount",
                reason: "must be between 50 and 999999999 CLP",
            })
        }
    }

    /// Amount in CLP.
    #[must_use]
    pub fn get(self) -> u32 {
        self.0
    }
}

impl TryFrom<u32> for Amount {
    type Error = Error;

    fn try_from(clp: u32) -> Result<Self> {
        Self::new(clp)
    }
}

impl From<Amount> for u32 {
    fn from(amount: Amount) -> Self {
        amount.0
    }
}

impl fmt::Display for Amount {
    /// Chilean format: `$12.000`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let digits = self.0.to_string();
        let mut out = String::with_capacity(digits.len() + 4);
        out.push('$');
        let len = digits.len();
        for (i, digit) in digits.chars().enumerate() {
            if i > 0 && (len - i) % 3 == 0 {
                out.push('.');
            }
            out.push(digit);
        }
        f.write_str(&out)
    }
}

/// Ticket (receipt) number: 1 to 6 ASCII alphanumeric characters.
///
/// It is sent padded with `0` on the left up to 6 characters, keeping the case
/// given by the caller.
///
/// ```
/// use transbank_pos::Ticket;
///
/// let ticket = Ticket::new("42")?;
/// assert_eq!(ticket.as_str(), "42");
/// assert_eq!(ticket.padded(), "000042");
/// assert!(Ticket::new("1234567").is_err());
/// # Ok::<(), transbank_pos::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "String", into = "String")
)]
pub struct Ticket(String);

impl Ticket {
    /// Validates a ticket number.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] if `text` is empty, longer than 6 characters or
    /// contains anything other than `[0-9A-Za-z]`.
    pub fn new(text: &str) -> Result<Self> {
        if text.is_empty() || text.len() > 6 {
            return Err(Error::InvalidArgument {
                field: "ticket",
                reason: "must have 1 to 6 characters",
            });
        }
        if !text.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
            return Err(Error::InvalidArgument {
                field: "ticket",
                reason: "must be ASCII alphanumeric",
            });
        }
        Ok(Self(text.to_owned()))
    }

    /// Ticket as given by the caller.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Ticket padded with `0` on the left up to 6 characters, as sent to the POS.
    #[must_use]
    pub fn padded(&self) -> String {
        format!("{:0>6}", self.0)
    }
}

impl TryFrom<&str> for Ticket {
    type Error = Error;

    fn try_from(text: &str) -> Result<Self> {
        Self::new(text)
    }
}

impl TryFrom<String> for Ticket {
    type Error = Error;

    fn try_from(text: String) -> Result<Self> {
        Self::new(&text)
    }
}

impl From<Ticket> for String {
    fn from(ticket: Ticket) -> Self {
        ticket.0
    }
}

impl fmt::Display for Ticket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Operation number of a sale, used to refund it: `1..=999_999` (sent with 6 digits).
///
/// ```
/// use transbank_pos::OperationNumber;
///
/// let op: OperationNumber = "000142".parse()?;
/// assert_eq!(op.get(), 142);
/// assert!(OperationNumber::new(0).is_err());
/// # Ok::<(), transbank_pos::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "u32", into = "u32")
)]
pub struct OperationNumber(u32);

impl OperationNumber {
    /// Maximum operation number (6 digits).
    pub const MAX: u32 = 999_999;

    /// Validates an operation number.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] if `number` is outside `1..=999_999`.
    pub fn new(number: u32) -> Result<Self> {
        if (1..=Self::MAX).contains(&number) {
            Ok(Self(number))
        } else {
            Err(Error::InvalidArgument {
                field: "operation",
                reason: "must be between 1 and 999999",
            })
        }
    }

    /// Operation number.
    #[must_use]
    pub fn get(self) -> u32 {
        self.0
    }
}

impl TryFrom<u32> for OperationNumber {
    type Error = Error;

    fn try_from(number: u32) -> Result<Self> {
        Self::new(number)
    }
}

impl From<OperationNumber> for u32 {
    fn from(op: OperationNumber) -> Self {
        op.0
    }
}

impl FromStr for OperationNumber {
    type Err = Error;

    /// Parses 1 to 6 digits (leading zeros allowed, as the POS reports them).
    fn from_str(text: &str) -> Result<Self> {
        match parse_digits::<u32>(text) {
            Some(number) if text.len() <= 6 => Self::new(number),
            _ => Err(Error::InvalidArgument {
                field: "operation",
                reason: "must be 1 to 6 digits",
            }),
        }
    }
}

/// Commerce code for multi-code sales: 1 to 12 digits; `"0"` is the main commerce.
///
/// ```
/// use transbank_pos::CommerceCode;
///
/// assert_eq!(CommerceCode::principal().as_str(), "0");
/// assert!(CommerceCode::new("597012345678").is_ok());
/// assert!(CommerceCode::new("59701234567890").is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "String", into = "String")
)]
pub struct CommerceCode(String);

impl CommerceCode {
    /// Validates a commerce code.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] unless `text` has 1 to 12 ASCII digits.
    pub fn new(text: &str) -> Result<Self> {
        if !text.is_empty() && text.len() <= 12 && text.bytes().all(|byte| byte.is_ascii_digit()) {
            Ok(Self(text.to_owned()))
        } else {
            Err(Error::InvalidArgument {
                field: "commerce_code",
                reason: "must be 1 to 12 digits",
            })
        }
    }

    /// The main commerce (`"0"`).
    #[must_use]
    pub fn principal() -> Self {
        Self("0".to_owned())
    }

    /// Commerce code as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommerceCode {
    type Error = Error;

    fn try_from(text: String) -> Result<Self> {
        Self::new(&text)
    }
}

impl From<CommerceCode> for String {
    fn from(code: CommerceCode) -> Self {
        code.0
    }
}

/// Date reported by the POS (no time zone).
///
/// ```
/// use transbank_pos::PosDate;
///
/// let date = PosDate::parse("22102025").unwrap();
/// assert_eq!((date.day, date.month, date.year), (22, 10, 2025));
/// assert_eq!(date.to_string(), "22-10-2025");
/// assert_eq!(PosDate::parse("00000000"), None);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct PosDate {
    /// Day of the month, `1..=31`.
    pub day: u8,
    /// Month, `1..=12`.
    pub month: u8,
    /// Year with 4 digits.
    pub year: u16,
}

impl PosDate {
    /// Parses `ddMMyyyy`. Returns `None` for anything that is not a valid date.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        if text.len() != 8 {
            return None;
        }
        let day: u8 = parse_digits(text.get(0..2)?)?;
        let month: u8 = parse_digits(text.get(2..4)?)?;
        let year: u16 = parse_digits(text.get(4..8)?)?;
        let valid = year > 0 && (1..=days_in_month(month, year)).contains(&day);
        valid.then_some(Self { day, month, year })
    }
}

fn days_in_month(month: u8, year: u16) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => 0,
    }
}

impl fmt::Display for PosDate {
    /// `dd-mm-yyyy`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}-{:02}-{:04}", self.day, self.month, self.year)
    }
}

/// Time of day reported by the POS (no time zone).
///
/// ```
/// use transbank_pos::PosTime;
///
/// let time = PosTime::parse("114016").unwrap();
/// assert_eq!(time.to_string(), "11:40:16");
/// assert_eq!(PosTime::parse("246000"), None);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct PosTime {
    /// Hour, `0..=23`.
    pub hour: u8,
    /// Minute, `0..=59`.
    pub minute: u8,
    /// Second, `0..=59`.
    pub second: u8,
}

impl PosTime {
    /// Parses `HHmmss`. Returns `None` for anything that is not a valid time.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        if text.len() != 6 {
            return None;
        }
        let hour: u8 = parse_digits(text.get(0..2)?)?;
        let minute: u8 = parse_digits(text.get(2..4)?)?;
        let second: u8 = parse_digits(text.get(4..6)?)?;
        (hour < 24 && minute < 60 && second < 60).then_some(Self {
            hour,
            minute,
            second,
        })
    }
}

impl fmt::Display for PosTime {
    /// `HH:mm:ss`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
    }
}

/// Card type reported by the POS.
///
/// Serialized (feature `serde`) as its code: `"CR"`, `"DB"`, `"PR"` or the raw text.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CardType {
    /// Credit card (`CR`).
    Credit,
    /// Debit card (`DB`).
    Debit,
    /// Prepaid card (`PR`).
    Prepaid,
    /// Any other code, as received.
    Other(String),
}

impl CardType {
    /// Maps the code sent by the POS.
    ///
    /// ```
    /// use transbank_pos::CardType;
    ///
    /// assert_eq!(CardType::from_code("CR"), CardType::Credit);
    /// assert_eq!(CardType::from_code("XX"), CardType::Other("XX".into()));
    /// ```
    #[must_use]
    pub fn from_code(code: &str) -> Self {
        match code {
            "CR" => Self::Credit,
            "DB" => Self::Debit,
            "PR" => Self::Prepaid,
            other => Self::Other(other.to_owned()),
        }
    }

    /// Code of the card type as sent by the POS.
    #[must_use]
    pub fn code(&self) -> &str {
        match self {
            Self::Credit => "CR",
            Self::Debit => "DB",
            Self::Prepaid => "PR",
            Self::Other(code) => code,
        }
    }
}

/// Card brand reported by the POS.
///
/// Serialized (feature `serde`) as its code: `"VI"`, `"MC"`… or the raw text.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CardBrand {
    /// Visa (`VI`).
    Visa,
    /// Mastercard (`MC`).
    Mastercard,
    /// American Express (`AX`).
    Amex,
    /// Redcompra (`DB`).
    Redcompra,
    /// Diners Club (`DC`).
    Diners,
    /// Magna (`MG`).
    Magna,
    /// Any other code, as received.
    Other(String),
}

impl CardBrand {
    /// Maps the abbreviation sent by the POS.
    ///
    /// ```
    /// use transbank_pos::CardBrand;
    ///
    /// assert_eq!(CardBrand::from_code("MC"), CardBrand::Mastercard);
    /// assert_eq!(CardBrand::from_code("TV").code(), "TV");
    /// ```
    #[must_use]
    pub fn from_code(code: &str) -> Self {
        match code {
            "VI" => Self::Visa,
            "MC" => Self::Mastercard,
            "AX" => Self::Amex,
            "DB" => Self::Redcompra,
            "DC" => Self::Diners,
            "MG" => Self::Magna,
            other => Self::Other(other.to_owned()),
        }
    }

    /// Abbreviation of the brand as sent by the POS.
    #[must_use]
    pub fn code(&self) -> &str {
        match self {
            Self::Visa => "VI",
            Self::Mastercard => "MC",
            Self::Amex => "AX",
            Self::Redcompra => "DB",
            Self::Diners => "DC",
            Self::Magna => "MG",
            Self::Other(code) => code,
        }
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for CardType {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.code())
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for CardBrand {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.code())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amount_bounds() {
        assert!(Amount::new(49).is_err());
        assert!(Amount::new(1_000_000_000).is_err());
        assert_eq!(Amount::new(50).unwrap().get(), 50);
        assert_eq!(Amount::new(999_999_999).unwrap().get(), 999_999_999);
        assert!(matches!(
            Amount::try_from(0),
            Err(Error::InvalidArgument {
                field: "amount",
                ..
            })
        ));
    }

    #[test]
    fn amount_display_uses_chilean_separator() {
        let cases = [
            (50, "$50"),
            (999, "$999"),
            (1_000, "$1.000"),
            (12_000, "$12.000"),
            (123_456, "$123.456"),
            (999_999_999, "$999.999.999"),
        ];
        for (clp, text) in cases {
            assert_eq!(Amount::new(clp).unwrap().to_string(), text);
        }
    }

    #[test]
    fn ticket_validation_and_padding() {
        assert_eq!(Ticket::new("42").unwrap().padded(), "000042");
        assert_eq!(Ticket::new("ABC123").unwrap().padded(), "ABC123");
        assert_eq!(Ticket::new("abc").unwrap().padded(), "000abc");
        for bad in ["", "1234567", "12-3", "añ", " 12", "12 "] {
            assert!(Ticket::new(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn operation_number_validation() {
        assert!(OperationNumber::new(0).is_err());
        assert!(OperationNumber::new(1_000_000).is_err());
        assert_eq!(OperationNumber::new(999_999).unwrap().get(), 999_999);
        assert_eq!("000135".parse::<OperationNumber>().unwrap().get(), 135);
        for bad in ["", "abc", "+12", "0000001", "000000"] {
            assert!(bad.parse::<OperationNumber>().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn commerce_code_validation() {
        assert!(CommerceCode::new("0").is_ok());
        assert!(CommerceCode::new("597029414300").is_ok());
        for bad in ["", "5970294143001", "59702941430a", "-1"] {
            assert!(CommerceCode::new(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn dates() {
        assert_eq!(
            PosDate::parse("29022024"),
            Some(PosDate {
                day: 29,
                month: 2,
                year: 2024
            })
        );
        for bad in [
            "29022025", "31042025", "00102025", "22132025", "00000000", "2210202", "22-10-25", "",
        ] {
            assert_eq!(PosDate::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn times() {
        assert_eq!(PosTime::parse("000000").unwrap().to_string(), "00:00:00");
        assert_eq!(PosTime::parse("235959").unwrap().to_string(), "23:59:59");
        for bad in ["240000", "236000", "235960", "11401", "1140166", "11:40:"] {
            assert_eq!(PosTime::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn card_codes_round_trip() {
        for code in ["CR", "DB", "PR", "ZZ"] {
            assert_eq!(CardType::from_code(code).code(), code);
        }
        for code in ["VI", "MC", "AX", "DB", "DC", "MG", "TV", "CA"] {
            assert_eq!(CardBrand::from_code(code).code(), code);
        }
    }
}
