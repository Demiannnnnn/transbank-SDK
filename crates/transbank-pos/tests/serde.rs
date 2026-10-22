//! Feature `serde`: responses serialize with their codes, input types deserialize
//! only valid values (validation cannot be bypassed through serde).
#![cfg(feature = "serde")]

use serde_json::{Value, json};
use transbank_pos::message::{parse_detail_entry, parse_sale};
use transbank_pos::{
    Amount, CardBrand, CardType, CommerceCode, OperationNumber, SaleRequest, SalesDetailMode,
    Ticket,
};

const SALE: &str = "0210|00|597029414300|IT750050|abc123|757752|12000|03|3334|9480|000135|CR|22102025|0000000000000000000|MC|22102025|114016|7|500|";

#[test]
fn sale_response_serializes_every_field() {
    let value = serde_json::to_value(parse_sale(SALE).unwrap()).unwrap();
    assert_eq!(value["function_code"], "0210");
    assert_eq!(value["response_code"], "Approved");
    assert_eq!(value["amount"], 12_000);
    assert_eq!(value["installments_number"], 3);
    assert_eq!(value["last4_digits"], "9480");
    assert_eq!(value["card_type"], "CR");
    assert_eq!(value["card_brand"], "MC");
    assert_eq!(
        value["accounting_date"],
        json!({"day": 22, "month": 10, "year": 2025})
    );
    assert_eq!(
        value["real_time"],
        json!({"hour": 11, "minute": 40, "second": 16})
    );
    assert_eq!(value["voucher"], Value::Null);
    assert_eq!(value["raw"], SALE);
}

#[test]
fn unknown_codes_serialize_as_their_text() {
    assert_eq!(
        serde_json::to_value(CardType::from_code("ZZ")).unwrap(),
        "ZZ"
    );
    assert_eq!(
        serde_json::to_value(CardBrand::from_code("TV")).unwrap(),
        "TV"
    );
    assert_eq!(serde_json::to_value(CardBrand::Redcompra).unwrap(), "DB");
}

#[test]
fn detail_entries_serialize() {
    let entry = parse_detail_entry("0261|00|597029414300|IT750050|abc123|757752|12000|9480|000135|CR|22102025|0000000000000000000|MC|22102025|114016|7|500|3334|03").unwrap();
    let value = serde_json::to_value(entry).unwrap();
    assert_eq!(value["installments_amount"], 3_334);
    assert_eq!(value["authorization_code"], "757752");
}

#[test]
fn input_types_round_trip() {
    let amount = Amount::new(12_000).unwrap();
    assert_eq!(serde_json::to_value(amount).unwrap(), 12_000);
    assert_eq!(
        serde_json::from_value::<Amount>(json!(12_000)).unwrap(),
        amount
    );

    let ticket = Ticket::new("abc12").unwrap();
    assert_eq!(serde_json::to_value(&ticket).unwrap(), "abc12");
    assert_eq!(
        serde_json::from_value::<Ticket>(json!("abc12")).unwrap(),
        ticket
    );

    assert_eq!(
        serde_json::from_value::<OperationNumber>(json!(142))
            .unwrap()
            .get(),
        142
    );
    assert_eq!(
        serde_json::from_value::<CommerceCode>(json!("597012345678"))
            .unwrap()
            .as_str(),
        "597012345678"
    );
    let mode: SalesDetailMode = serde_json::from_value(json!("SendToHost")).unwrap();
    assert_eq!(mode, SalesDetailMode::SendToHost);
}

#[test]
fn deserialization_validates() {
    assert!(serde_json::from_value::<Amount>(json!(49)).is_err());
    assert!(serde_json::from_value::<Amount>(json!(1_000_000_000)).is_err());
    assert!(serde_json::from_value::<Ticket>(json!("1234567")).is_err());
    assert!(serde_json::from_value::<Ticket>(json!("ab-1")).is_err());
    assert!(serde_json::from_value::<OperationNumber>(json!(0)).is_err());
    assert!(serde_json::from_value::<CommerceCode>(json!("12a")).is_err());
}

#[test]
fn sale_request_round_trips_and_validates() {
    let request = SaleRequest::new(5_000, "T1").unwrap().with_voucher(true);
    let value = serde_json::to_value(&request).unwrap();
    assert_eq!(
        value,
        json!({"amount": 5_000, "ticket": "T1", "voucher": true})
    );
    assert_eq!(
        serde_json::from_value::<SaleRequest>(value).unwrap(),
        request
    );

    let without_voucher: SaleRequest =
        serde_json::from_value(json!({"amount": 5_000, "ticket": "T1"})).unwrap();
    assert!(!without_voucher.voucher());

    assert!(serde_json::from_value::<SaleRequest>(json!({"amount": 10, "ticket": "T1"})).is_err());
    assert!(serde_json::from_value::<SaleRequest>(json!({"amount": 5_000, "ticket": ""})).is_err());
}
