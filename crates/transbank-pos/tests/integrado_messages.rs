//! POS Integrado responses: every fixture parses (QA-01) and no parser panics with
//! arbitrary input (PROTO-04).

use proptest::prelude::*;
use transbank_pos::message::{
    parse_close, parse_detail_entry, parse_intermediate, parse_load_keys,
    parse_multicode_detail_entry, parse_multicode_sale, parse_refund, parse_sale, parse_totals,
};
use transbank_pos::{ResponseCode, Result};

const FIXTURES: &str = include_str!("fixtures/integrado.txt");

/// Parses a payload with the parser of its function code and returns its response code.
fn parse_any(payload: &str) -> Result<ResponseCode> {
    let code = match payload.get(..4).unwrap_or("") {
        "0210" | "0260" => parse_sale(payload)?.response_code,
        "0271" | "0281" => parse_multicode_sale(payload)?.sale.response_code,
        "0261" => parse_detail_entry(payload)?.response_code,
        "0291" => parse_multicode_detail_entry(payload)?.sale.response_code,
        "0810" => parse_load_keys(payload)?.response_code,
        "0510" => parse_close(payload)?.response_code,
        "0710" => parse_totals(payload)?.response_code,
        "1210" => parse_refund(payload)?.response_code,
        _ => ResponseCode::Other(u16::MAX),
    };
    Ok(code)
}

#[test]
fn every_fixture_parses() {
    let payloads: Vec<&str> = FIXTURES
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    assert_eq!(payloads.len(), 8);
    for payload in payloads {
        let code = parse_any(payload).unwrap_or_else(|err| panic!("{payload}: {err}"));
        assert_ne!(code, ResponseCode::Other(u16::MAX), "{payload}");
    }
}

fn payload_strategy() -> impl Strategy<Value = String> {
    let field = prop_oneof![
        Just(String::new()),
        "[0-9]{1,12}",
        "[ -~]{0,10}",
        any::<String>(),
    ];
    (
        prop::sample::select(vec![
            "0210", "0260", "0271", "0281", "0261", "0291", "0810", "0510", "0710", "1210", "0900",
            "9999", "",
        ]),
        prop::collection::vec(field, 0..25),
    )
        .prop_map(|(code, fields)| {
            let mut payload = code.to_owned();
            for field in fields {
                payload.push('|');
                payload.push_str(&field);
            }
            payload
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn parsers_never_panic(payload in payload_strategy()) {
        let _ = parse_sale(&payload);
        let _ = parse_multicode_sale(&payload);
        let _ = parse_detail_entry(&payload);
        let _ = parse_multicode_detail_entry(&payload);
        let _ = parse_load_keys(&payload);
        let _ = parse_close(&payload);
        let _ = parse_totals(&payload);
        let _ = parse_refund(&payload);
        let _ = parse_intermediate(&payload);
    }

    #[test]
    fn arbitrary_text_never_panics(payload in any::<String>()) {
        let _ = parse_any(&payload);
        let _ = parse_intermediate(&payload);
    }
}
