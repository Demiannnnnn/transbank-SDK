//! Property tests of the domain types, request payloads and response codes: for every
//! valid input the result is well formed, and every invalid input is rejected.

use proptest::prelude::*;
use transbank_pos::message::{
    multicode_sale_payload, parse_intermediate, parse_refund, parse_sale, parse_totals,
    refund_payload, sale_payload,
};
use transbank_pos::protocol::{FrameDecoder, LinkEvent, encode_frame};
use transbank_pos::{
    Amount, CommerceCode, IntermediateStatus, MultiCodeSaleRequest, OperationNumber, PosDate,
    PosTime, ResponseCode, SaleRequest, Ticket,
};

fn ticket_text() -> impl Strategy<Value = String> {
    "[0-9A-Za-z]{1,6}"
}

fn days_in_month(month: u8, year: u16) -> u8 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn valid_date() -> impl Strategy<Value = (u8, u8, u16)> {
    (1u8..=12, 1u16..=9999).prop_flat_map(|(month, year)| {
        (1..=days_in_month(month, year)).prop_map(move |day| (day, month, year))
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn amount_accepts_exactly_the_sale_range(clp in any::<u32>()) {
        let valid = (50..=999_999_999).contains(&clp);
        prop_assert_eq!(Amount::new(clp).is_ok(), valid);
        if valid {
            prop_assert_eq!(Amount::new(clp).unwrap().get(), clp);
        }
    }

    #[test]
    fn amount_display_round_trips(clp in 50u32..=999_999_999) {
        let text = Amount::new(clp).unwrap().to_string();
        prop_assert!(text.starts_with('$'));
        let digits: String = text.chars().filter(char::is_ascii_digit).collect();
        prop_assert_eq!(digits.parse::<u32>().unwrap(), clp);
        // Groups of exactly three digits after the first separator.
        for group in text.trim_start_matches('$').split('.').skip(1) {
            prop_assert_eq!(group.len(), 3);
        }
    }

    #[test]
    fn ticket_accepts_exactly_1_to_6_alphanumerics(text in "\\PC{0,8}") {
        let valid = (1..=6).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_alphanumeric());
        prop_assert_eq!(Ticket::new(&text).is_ok(), valid);
    }

    #[test]
    fn ticket_padding_keeps_the_text(text in ticket_text()) {
        let padded = Ticket::new(&text).unwrap().padded();
        prop_assert_eq!(padded.len(), 6);
        prop_assert!(padded.ends_with(text.as_str()));
        prop_assert!(padded[..6 - text.len()].bytes().all(|b| b == b'0'));
    }

    #[test]
    fn operation_number_from_str_round_trips(number in 1u32..=999_999) {
        let padded = format!("{number:06}");
        prop_assert_eq!(padded.parse::<OperationNumber>().unwrap().get(), number);
        prop_assert_eq!(number.to_string().parse::<OperationNumber>().unwrap().get(), number);
    }

    #[test]
    fn commerce_code_accepts_1_to_12_digits(text in "[0-9a]{0,14}") {
        let valid = (1..=12).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_digit());
        prop_assert_eq!(CommerceCode::new(&text).is_ok(), valid);
    }

    #[test]
    fn valid_dates_parse_and_display((day, month, year) in valid_date()) {
        let date = PosDate::parse(&format!("{day:02}{month:02}{year:04}")).unwrap();
        prop_assert_eq!((date.day, date.month, date.year), (day, month, year));
        prop_assert_eq!(date.to_string(), format!("{day:02}-{month:02}-{year:04}"));
    }

    #[test]
    fn date_parse_never_panics(text in "\\PC{0,10}") {
        if let Some(date) = PosDate::parse(&text) {
            prop_assert!((1..=12).contains(&date.month));
            prop_assert!(date.day >= 1 && date.day <= days_in_month(date.month, date.year));
        }
    }

    #[test]
    fn time_parse_accepts_exactly_valid_times(h in 0u8..30, m in 0u8..70, s in 0u8..70) {
        let parsed = PosTime::parse(&format!("{h:02}{m:02}{s:02}"));
        prop_assert_eq!(parsed.is_some(), h < 24 && m < 60 && s < 60);
    }

    /// Every valid sale request builds a payload that encodes to a frame and decodes
    /// back, with the amount and ticket in their fixed-width fields.
    #[test]
    fn sale_payloads_are_well_formed(
        clp in 50u32..=999_999_999,
        ticket in ticket_text(),
        voucher in any::<bool>(),
        status in any::<bool>(),
    ) {
        let request = SaleRequest::new(clp, &ticket).unwrap().with_voucher(voucher);
        let payload = sale_payload(&request, status);
        let fields: Vec<&str> = payload.split('|').collect();
        prop_assert_eq!(fields.len(), 6);
        prop_assert_eq!(fields[0], "0200");
        prop_assert_eq!(fields[1].len(), 9);
        prop_assert_eq!(fields[1].parse::<u32>().unwrap(), clp);
        prop_assert_eq!(fields[2], Ticket::new(&ticket).unwrap().padded());
        prop_assert_eq!(fields[3], "");
        prop_assert_eq!(fields[4], if voucher { "1" } else { "0" });
        prop_assert_eq!(fields[5], if status { "1" } else { "0" });

        let frame = encode_frame(&payload).unwrap();
        let mut events = Vec::new();
        FrameDecoder::new().push(&frame, &mut events);
        prop_assert_eq!(events, vec![LinkEvent::Frame(payload.into_bytes())]);
    }

    #[test]
    fn multicode_payloads_end_with_the_commerce(
        clp in 50u32..=999_999_999,
        ticket in ticket_text(),
        commerce in "[0-9]{1,12}",
    ) {
        let request = MultiCodeSaleRequest::new(clp, &ticket, &commerce).unwrap();
        let payload = multicode_sale_payload(&request, false);
        prop_assert!(payload.starts_with("0270|"));
        let suffix = format!("|{commerce}");
        prop_assert!(payload.ends_with(&suffix));
        prop_assert!(encode_frame(&payload).is_ok());
    }

    #[test]
    fn refund_payloads_have_six_digits(number in 1u32..=999_999) {
        let payload = refund_payload(OperationNumber::new(number).unwrap());
        prop_assert_eq!(payload, format!("1200|{number:06}|"));
    }

    #[test]
    fn response_codes_round_trip(code in any::<u16>()) {
        let parsed = ResponseCode::from_code(code);
        prop_assert_eq!(parsed.code(), code);
        prop_assert!(!parsed.message().is_empty());
        prop_assert_eq!(parsed.to_string(), format!("{code:02} {}", parsed.message()));
        prop_assert_eq!(parsed.is_approved(), code == 0 || code == 90);
    }

    #[test]
    fn any_numeric_response_code_parses(code in 0u16..=999) {
        let sale = parse_sale(&format!("0210|{code}|597029414300|T")).unwrap();
        prop_assert_eq!(sale.response_code.code(), code);
        let totals = parse_totals(&format!("0710|{code:02}|0|0||")).unwrap();
        prop_assert_eq!(totals.response_code.code(), code);
        let refund = parse_refund(&format!("1210|{code}|||||")).unwrap();
        prop_assert_eq!(refund.is_approved(), code == 0 || code == 90);
    }

    #[test]
    fn intermediate_status_is_never_an_error(text in "[ -{}~]{0,6}") {
        let status = parse_intermediate(&format!("0900|{text}")).unwrap();
        match status {
            IntermediateStatus::Code(code) => {
                prop_assert!(!matches!(code, ResponseCode::Other(_)));
                prop_assert_eq!(text.trim().parse::<u16>().ok(), Some(code.code()));
            }
            IntermediateStatus::Unknown(raw) => prop_assert_eq!(raw, text.trim()),
            _ => prop_assert!(false, "new variant"),
        }
    }

    #[test]
    fn sale_fields_round_trip(
        amount in 0u32..=999_999_999,
        installments in 0u32..=48,
        tip in 0u32..=100_000,
        last4 in "[0-9]{4}",
        operation in "[0-9]{6}",
    ) {
        let payload = format!(
            "0210|00|597029414300|T1|ABC|123456|{amount}|{installments:02}|0|{last4}|{operation}|CR|01012026|0000|MC|01012026|120000|1|{tip}|"
        );
        let sale = parse_sale(&payload).unwrap();
        prop_assert_eq!(sale.amount, Some(amount));
        prop_assert_eq!(sale.installments_number, Some(installments));
        prop_assert_eq!(sale.tip, Some(tip));
        prop_assert_eq!(sale.last4_digits, Some(last4));
        prop_assert_eq!(sale.operation_number, Some(operation));
        prop_assert_eq!(sale.raw, payload);
    }
}
