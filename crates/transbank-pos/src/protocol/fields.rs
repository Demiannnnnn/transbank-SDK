//! Field helpers over a payload split by `|` (`docs/protocol-spec.md` §5, PROTO-05).

use super::types::{CardBrand, CardType, PosDate, PosTime, parse_digits};
use crate::codes::ResponseCode;
use crate::error::{Error, Result};

/// A received payload split into fields.
///
/// Absent or empty fields are `None`; a non-empty numeric field that does not parse
/// is an [`Error::Parse`] with the field name. No default values are invented.
pub(crate) struct Fields<'a> {
    payload: &'a str,
    fields: Vec<&'a str>,
}

impl<'a> Fields<'a> {
    pub(crate) fn new(payload: &'a str) -> Self {
        Self {
            payload,
            fields: payload.split('|').collect(),
        }
    }

    /// Raw payload.
    pub(crate) fn payload(&self) -> &'a str {
        self.payload
    }

    /// Function code (field 0), trimmed.
    pub(crate) fn function_code(&self) -> &'a str {
        self.fields.first().map_or("", |field| field.trim())
    }

    /// Fails with `UnexpectedResponse` unless the function code is one of `expected`.
    /// The error reports the first expected code.
    pub(crate) fn expect_function(&self, expected: &[&'static str]) -> Result<&'a str> {
        let received = self.function_code();
        if expected.contains(&received) {
            return Ok(received);
        }
        Err(Error::UnexpectedResponse {
            expected: expected.first().copied().unwrap_or(""),
            received: received.to_owned(),
            payload: self.payload.to_owned(),
        })
    }

    fn trimmed(&self, index: usize) -> Option<&'a str> {
        self.fields
            .get(index)
            .map(|field| field.trim())
            .filter(|field| !field.is_empty())
    }

    /// Text field, trimmed; empty ⇒ `None`.
    pub(crate) fn str_at(&self, index: usize) -> Option<String> {
        self.trimmed(index).map(str::to_owned)
    }

    /// Text field kept as received (not trimmed, e.g. a voucher of fixed-width
    /// lines); `None` if absent or blank.
    pub(crate) fn raw_at(&self, index: usize) -> Option<String> {
        self.trimmed(index)?;
        self.fields.get(index).map(|field| (*field).to_owned())
    }

    fn number_at<T: std::str::FromStr>(
        &self,
        index: usize,
        name: &'static str,
    ) -> Result<Option<T>> {
        let Some(text) = self.trimmed(index) else {
            return Ok(None);
        };
        parse_digits(text).map(Some).ok_or_else(|| Error::Parse {
            field: name,
            value: text.to_owned(),
            payload: self.payload.to_owned(),
        })
    }

    /// Unsigned 32-bit number.
    pub(crate) fn u32_at(&self, index: usize, name: &'static str) -> Result<Option<u32>> {
        self.number_at(index, name)
    }

    /// Unsigned 64-bit number.
    pub(crate) fn u64_at(&self, index: usize, name: &'static str) -> Result<Option<u64>> {
        self.number_at(index, name)
    }

    /// Date `ddMMyyyy`; invalid ⇒ `None` (the raw text stays in `raw`).
    pub(crate) fn date_at(&self, index: usize) -> Option<PosDate> {
        self.trimmed(index).and_then(PosDate::parse)
    }

    /// Time `HHmmss`; invalid ⇒ `None`.
    pub(crate) fn time_at(&self, index: usize) -> Option<PosTime> {
        self.trimmed(index).and_then(PosTime::parse)
    }

    pub(crate) fn card_type_at(&self, index: usize) -> Option<CardType> {
        self.trimmed(index).map(CardType::from_code)
    }

    pub(crate) fn card_brand_at(&self, index: usize) -> Option<CardBrand> {
        self.trimmed(index).map(CardBrand::from_code)
    }

    /// Response code (field 1): required and numeric.
    pub(crate) fn required_code(&self) -> Result<ResponseCode> {
        let text = self.trimmed(1).unwrap_or("");
        parse_digits::<u16>(text)
            .map(ResponseCode::from_code)
            .ok_or_else(|| Error::Parse {
                field: "response_code",
                value: text.to_owned(),
                payload: self.payload.to_owned(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_and_empty_are_none() {
        let fields = Fields::new("0210|00| |x");
        assert_eq!(fields.str_at(2), None);
        assert_eq!(fields.str_at(3).as_deref(), Some("x"));
        assert_eq!(fields.str_at(9), None);
        assert_eq!(fields.u32_at(9, "amount").unwrap(), None);
        assert_eq!(fields.date_at(9), None);
    }

    #[test]
    fn numbers_are_strict() {
        let fields = Fields::new("0210|00| 0042 |12a00|-1|99999999999");
        assert_eq!(fields.u32_at(2, "n").unwrap(), Some(42));
        for index in [3, 4, 5] {
            assert!(matches!(
                fields.u32_at(index, "amount"),
                Err(Error::Parse {
                    field: "amount",
                    ..
                })
            ));
        }
        assert_eq!(fields.u64_at(5, "total").unwrap(), Some(99_999_999_999));
    }

    #[test]
    fn response_code_is_required() {
        assert_eq!(
            Fields::new("0210|07").required_code().unwrap(),
            ResponseCode::CanceledAtPos
        );
        for payload in ["0210", "0210|", "0210|x"] {
            assert!(matches!(
                Fields::new(payload).required_code(),
                Err(Error::Parse {
                    field: "response_code",
                    ..
                })
            ));
        }
    }

    #[test]
    fn function_code_check() {
        let fields = Fields::new("0810|00");
        assert_eq!(fields.expect_function(&["0810"]).unwrap(), "0810");
        assert!(matches!(
            fields.expect_function(&["0210", "0260"]),
            Err(Error::UnexpectedResponse {
                expected: "0210",
                ref received,
                ..
            }) if received == "0810"
        ));
    }

    #[test]
    fn raw_keeps_spaces() {
        let fields = Fields::new("0510|00|1|2| AB  |   ");
        assert_eq!(fields.raw_at(4).as_deref(), Some(" AB  "));
        assert_eq!(fields.raw_at(5), None);
    }
}
