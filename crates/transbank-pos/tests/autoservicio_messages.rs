//! POS Autoservicio messages and parser tests (PROTO-01, PROTO-05, PROTO-06, PROTO-08).

use transbank_pos::message::{
    MultiCodeSaleRequest, SaleRequest, initialization_result_payload, initialize_payload,
    parse_initialization, parse_self_service_multicode_sale, parse_self_service_sale,
    self_service_close_payload, self_service_last_sale_payload,
    self_service_multicode_sale_payload, self_service_refund_payload, self_service_sale_payload,
};
use transbank_pos::protocol::encode_frame;
use transbank_pos::{CardBrand, CardType, Error, PosDate, PosTime, ResponseCode};

const FIXTURES: &str = include_str!("fixtures/autoservicio.txt");

#[test]
fn payloads_and_frames_byte_by_byte() {
    // 0070
    let payload = initialize_payload();
    assert_eq!(payload, "0070");
    let frame = encode_frame(&payload).unwrap();
    assert_eq!(frame, [0x02, b'0', b'0', b'7', b'0', 0x03, 0x04]);

    // 0080
    let payload = initialization_result_payload();
    assert_eq!(payload, "0080");
    let frame = encode_frame(&payload).unwrap();
    assert_eq!(frame, [0x02, b'0', b'0', b'8', b'0', 0x03, 0x0B]);

    // 0200
    let req = SaleRequest::new(10_000, "123456")
        .unwrap()
        .with_voucher(true);
    let payload = self_service_sale_payload(&req, false);
    assert_eq!(payload, "0200|000010000|123456|1|0");
    let frame = encode_frame(&payload).unwrap();
    assert_eq!(
        frame,
        [
            0x02, b'0', b'2', b'0', b'0', b'|', b'0', b'0', b'0', b'0', b'1', b'0', b'0', b'0',
            b'0', b'|', b'1', b'2', b'3', b'4', b'5', b'6', b'|', b'1', b'|', b'0', 0x03, 0x36,
        ]
    );

    // 0270
    let mreq = MultiCodeSaleRequest::new(10_000, "123456", "597012345678")
        .unwrap()
        .with_voucher(false);
    let payload = self_service_multicode_sale_payload(&mreq, true);
    assert_eq!(payload, "0270|000010000|123456|0|1|597012345678");
    let frame = encode_frame(&payload).unwrap();
    assert_eq!(
        frame,
        [
            0x02, b'0', b'2', b'7', b'0', b'|', b'0', b'0', b'0', b'0', b'1', b'0', b'0', b'0',
            b'0', b'|', b'1', b'2', b'3', b'4', b'5', b'6', b'|', b'0', b'|', b'1', b'|', b'5',
            b'9', b'7', b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', 0x03, 0x4E,
        ]
    );

    // 0250
    assert_eq!(self_service_last_sale_payload(false), "0250|0");
    assert_eq!(self_service_last_sale_payload(true), "0250|1");
    let frame = encode_frame(&self_service_last_sale_payload(true)).unwrap();
    assert_eq!(
        frame,
        [0x02, b'0', b'2', b'5', b'0', b'|', b'1', 0x03, 0x49]
    );

    // 1200 exact, without fields
    let payload = self_service_refund_payload();
    assert_eq!(payload, "1200");
    let frame = encode_frame(&payload).unwrap();
    assert_eq!(frame, [0x02, b'1', b'2', b'0', b'0', 0x03, 0x00]);

    // 0500
    assert_eq!(self_service_close_payload(false), "0500|0");
    assert_eq!(self_service_close_payload(true), "0500|1");
    let frame = encode_frame(&self_service_close_payload(false)).unwrap();
    assert_eq!(
        frame,
        [0x02, b'0', b'5', b'0', b'0', b'|', b'0', 0x03, 0x4A]
    );
}

fn fixture_lines() -> Vec<&'static str> {
    FIXTURES
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
}

#[test]
fn fixture_standard_sale() {
    let lines = fixture_lines();
    assert_eq!(lines.len(), 4);

    let sale = parse_self_service_sale(lines[0]).unwrap();
    assert_eq!(sale.function_code, "0210");
    assert_eq!(sale.response_code, ResponseCode::Approved);
    assert_eq!(sale.commerce_code.as_deref(), Some("597029414300"));
    assert_eq!(sale.terminal_id.as_deref(), Some("IM750164"));
    assert_eq!(sale.ticket.as_deref(), Some("123456"));
    assert_eq!(sale.authorization_code.as_deref(), Some("776549"));
    assert_eq!(sale.amount, Some(10000));
    assert_eq!(sale.last4_digits.as_deref(), Some("6590"));
    assert_eq!(sale.operation_number.as_deref(), Some("58"));
    assert_eq!(sale.card_type, Some(CardType::Credit));
    assert_eq!(sale.accounting_date, None);
    assert_eq!(sale.account_number, None);
    assert_eq!(sale.card_brand, Some(CardBrand::Visa));
    assert_eq!(
        sale.real_date,
        Some(PosDate {
            day: 18,
            month: 3,
            year: 2026
        })
    );
    assert_eq!(
        sale.real_time,
        Some(PosTime {
            hour: 12,
            minute: 35,
            second: 6
        })
    );
    assert_eq!(sale.voucher, None);
    assert_eq!(sale.installments_type.as_deref(), Some("03"));
    assert_eq!(sale.installments_number, Some(3));
    assert_eq!(sale.installments_amount, Some(3334));
    assert_eq!(
        sale.installments_type_description.as_deref(),
        Some("CUOTAS SIN INTERES")
    );
    assert!(sale.is_approved());
}

#[test]
fn fixture_initialization_response() {
    let lines = fixture_lines();
    let init = parse_initialization(lines[1]).unwrap();
    assert_eq!(init.response_code, ResponseCode::InitializationOk);
    assert_eq!(
        init.date,
        Some(PosDate {
            day: 3,
            month: 2,
            year: 2026
        })
    );
    assert_eq!(
        init.time,
        Some(PosTime {
            hour: 11,
            minute: 15,
            second: 43
        })
    );
    assert!(init.is_approved());
}

#[test]
fn fixture_sale_with_voucher_and_account_number() {
    let lines = fixture_lines();
    let sale_v = parse_self_service_sale(lines[2]).unwrap();
    assert_eq!(
        sale_v.account_number.as_deref(),
        Some("0000000000000000000")
    );
    assert_eq!(sale_v.voucher.as_ref().map(String::len), Some(120));
    let voucher_lines = sale_v.voucher_lines().unwrap();
    assert_eq!(voucher_lines.len(), 3);
    assert_eq!(voucher_lines[0].len(), 40);
}

#[test]
fn fixture_multicode_sale() {
    let lines = fixture_lines();
    let multi = parse_self_service_multicode_sale(lines[3]).unwrap();
    assert_eq!(multi.sale.amount, Some(10000));
    assert_eq!(
        multi.commerce_provider_code.as_deref(),
        Some("597012345678")
    );
    assert!(multi.is_approved());
}

#[test]
fn absent_fields_are_none() {
    let sale = parse_self_service_sale("0210|00|597029414300|IM750164").unwrap();
    assert_eq!(sale.ticket, None);
    assert_eq!(sale.authorization_code, None);
    assert_eq!(sale.amount, None);
    assert_eq!(sale.last4_digits, None);
    assert_eq!(sale.operation_number, None);
    assert_eq!(sale.voucher, None);
    assert_eq!(sale.installments_number, None);
}

#[test]
fn invalid_number_is_parse_error() {
    let err = parse_self_service_sale(
        "0210|00|597029414300|IM750164|123456|776549|INV|6590|58|CR|||VI|18032026|123506||03|03|3334|CUOTAS",
    )
    .unwrap_err();
    assert!(matches!(
        err,
        Error::Parse {
            field: "amount",
            ..
        }
    ));

    let err2 = parse_self_service_sale(
        "0210|00|597029414300|IM750164|123456|776549|1000|6590|58|CR|||VI|18032026|123506||03|INV|3334|CUOTAS",
    )
    .unwrap_err();
    assert!(matches!(
        err2,
        Error::Parse {
            field: "installments_number",
            ..
        }
    ));
}

#[test]
fn voucher_not_multiple_of_40_yields_none() {
    let mut payload =
        "0210|00|597029414300|IM750164|123456|776549|10000|6590|58|CR|||VI|18032026|123506|"
            .to_owned();
    payload.push_str("short_voucher|03|03|3334|CUOTAS");
    let sale = parse_self_service_sale(&payload).unwrap();
    assert_eq!(sale.voucher.as_deref(), Some("short_voucher"));
    assert_eq!(sale.voucher_lines(), None);
}

#[test]
fn leading_zeros_preserved() {
    let sale = parse_self_service_sale(
        "0210|00|597029414300|IM750164|000123|007749|10000|0090|000058|CR|||VI|18032026|123506||03|03|3334|CUOTAS",
    )
    .unwrap();
    assert_eq!(sale.ticket.as_deref(), Some("000123"));
    assert_eq!(sale.authorization_code.as_deref(), Some("007749"));
    assert_eq!(sale.last4_digits.as_deref(), Some("0090"));
    assert_eq!(sale.operation_number.as_deref(), Some("000058"));
}

#[test]
fn debug_redacts_account_number_and_voucher() {
    let sale = parse_self_service_sale(
        "0210|00|597029414300|IM750164|123456|776549|10000|6590|58|CR|18032026|SECRET_ACCOUNT|VI|18032026|123506|SECRET_VOUCHER_TEXT|03|03|3334|CUOTAS",
    )
    .unwrap();
    let debug = format!("{sale:?}");
    assert!(!debug.contains("SECRET_ACCOUNT"));
    assert!(!debug.contains("SECRET_VOUCHER_TEXT"));
    assert!(debug.contains("<redacted "));
}

#[test]
fn approval_decisions() {
    // 0210 with 90 is NOT approved
    let sale_90 = parse_self_service_sale("0210|90|597029414300|IM750164").unwrap();
    assert_eq!(sale_90.response_code, ResponseCode::InitializationOk);
    assert!(!sale_90.is_approved());

    // 1080 with 00 is NOT approved
    let init_00 = parse_initialization("1080|00|03022026|111543").unwrap();
    assert_eq!(init_00.response_code, ResponseCode::Approved);
    assert!(!init_00.is_approved());

    // 1080 with 90 IS approved
    let init_90 = parse_initialization("1080|90|03022026|111543").unwrap();
    assert_eq!(init_90.response_code, ResponseCode::InitializationOk);
    assert!(init_90.is_approved());
}

#[test]
fn unexpected_response_error() {
    let err = parse_self_service_sale("0710|00|1|100").unwrap_err();
    assert!(matches!(err, Error::UnexpectedResponse { .. }));

    let err_init = parse_initialization("0210|00|comm|term").unwrap_err();
    assert!(matches!(err_init, Error::UnexpectedResponse { .. }));

    let err_multi = parse_self_service_multicode_sale("0210|00|comm|term").unwrap_err();
    assert!(matches!(err_multi, Error::UnexpectedResponse { .. }));
}

#[test]
#[cfg(feature = "serde")]
fn serde_support() {
    let sale = parse_self_service_sale(
        "0210|00|597029414300|IM750164|123456|776549|10000|6590|58|CR|||VI|18032026|123506||03|03|3334|CUOTAS",
    )
    .unwrap();
    let json = serde_json::to_string(&sale).unwrap();
    assert!(json.contains("\"function_code\":\"0210\""));
    assert!(json.contains("\"amount\":10000"));

    let init = parse_initialization("1080|90|03022026|111543").unwrap();
    let json_init = serde_json::to_string(&init).unwrap();
    assert!(json_init.contains("\"response_code\":\"InitializationOk\""));
}
