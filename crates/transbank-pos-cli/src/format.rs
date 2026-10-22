//! Human-readable output in Spanish (pure functions, unit tested).

use std::fmt::Write as _;

use transbank_pos::serial::PortInfo;
use transbank_pos::{
    CardBrand, CardType, CloseResponse, DetailEntry, LoadKeysResponse, MultiCodeSaleResponse,
    RefundResponse, ResponseCode, SaleResponse, TotalsResponse,
};

/// Terminal ids of the simulator start with this prefix (security-model §4).
const SIMULATOR_PREFIX: &str = "SIM";

/// `$12.000`.
pub(crate) fn clp(amount: u64) -> String {
    let digits = amount.to_string();
    let len = digits.len();
    let mut out = String::with_capacity(len + 4);
    out.push('$');
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            out.push('.');
        }
        out.push(digit);
    }
    out
}

pub(crate) fn brand_name(brand: &CardBrand) -> &str {
    match brand {
        CardBrand::Visa => "VISA",
        CardBrand::Mastercard => "Mastercard",
        CardBrand::Amex => "American Express",
        CardBrand::Redcompra => "Redcompra",
        CardBrand::Diners => "Diners Club",
        CardBrand::Magna => "Magna",
        other => other.code(),
    }
}

pub(crate) fn card_type_name(card_type: &CardType) -> &str {
    match card_type {
        CardType::Credit => "crédito",
        CardType::Debit => "débito",
        CardType::Prepaid => "prepago",
        other => other.code(),
    }
}

fn simulator_tag(terminal_id: Option<&str>) -> &'static str {
    if terminal_id.is_some_and(|id| id.starts_with(SIMULATOR_PREFIX)) {
        "[SIMULADOR] "
    } else {
        ""
    }
}

fn rejected(what: &str, code: ResponseCode) -> String {
    format!("{what} RECHAZADA — {code}")
}

/// "Venta APROBADA — autorización 757752 — $12.000 — VISA crédito ···9480 — 3 cuotas
/// de $3.334 — operación 000135".
pub(crate) fn format_sale(title: &str, sale: &SaleResponse) -> String {
    let tag = simulator_tag(sale.terminal_id.as_deref());
    if !sale.is_approved() {
        return format!("{tag}{}", rejected(title, sale.response_code));
    }
    let mut parts = vec![format!("{title} APROBADA")];
    if let Some(code) = &sale.authorization_code {
        parts.push(format!("autorización {code}"));
    }
    if let Some(amount) = sale.amount {
        parts.push(clp(u64::from(amount)));
    }
    let card: Vec<&str> = [
        sale.card_brand.as_ref().map(brand_name),
        sale.card_type.as_ref().map(card_type_name),
    ]
    .into_iter()
    .flatten()
    .collect();
    let mut card = card.join(" ");
    if let Some(last4) = &sale.last4_digits {
        if !card.is_empty() {
            card.push(' ');
        }
        card.push_str("···");
        card.push_str(last4);
    }
    if !card.is_empty() {
        parts.push(card);
    }
    if let (Some(count), Some(each)) = (sale.installments_number, sale.installments_amount) {
        if count > 1 {
            parts.push(format!("{count} cuotas de {}", clp(u64::from(each))));
        }
    }
    if let Some(operation) = &sale.operation_number {
        parts.push(format!("operación {operation}"));
    }
    format!("{tag}{}", parts.join(" — "))
}

pub(crate) fn format_multicode_sale(title: &str, sale: &MultiCodeSaleResponse) -> String {
    let mut text = format_sale(title, &sale.sale);
    if sale.is_approved() {
        if let Some(code) = &sale.commerce_provider_code {
            let _ = write!(text, " — comercio {code}");
        }
    }
    text
}

pub(crate) fn format_refund(refund: &RefundResponse) -> String {
    let tag = simulator_tag(refund.terminal_id.as_deref());
    if !refund.is_approved() {
        return format!("{tag}{}", rejected("Anulación", refund.response_code));
    }
    format!(
        "{tag}Anulación APROBADA — autorización {} — operación {}",
        refund.authorization_code.as_deref().unwrap_or("-"),
        refund.operation_number.as_deref().unwrap_or("-")
    )
}

pub(crate) fn format_totals(totals: &TotalsResponse) -> String {
    if !totals.is_approved() {
        return format!("Totales: {}", totals.response_code);
    }
    let count = totals.tx_count.unwrap_or(0);
    let noun = if count == 1 { "venta" } else { "ventas" };
    format!(
        "{count} {noun} — total {}",
        clp(totals.tx_total.unwrap_or(0))
    )
}

pub(crate) fn format_detail_entry(entry: &DetailEntry) -> String {
    let card: Vec<&str> = [
        entry.card_brand.as_ref().map(brand_name),
        entry.card_type.as_ref().map(card_type_name),
    ]
    .into_iter()
    .flatten()
    .collect();
    format!(
        "ticket {:>6}  {:>14}  operación {}  autorización {}  {}",
        entry.ticket.as_deref().unwrap_or("-"),
        clp(u64::from(entry.amount.unwrap_or(0))),
        entry.operation_number.as_deref().unwrap_or("-"),
        entry.authorization_code.as_deref().unwrap_or("-"),
        card.join(" ")
    )
}

pub(crate) fn format_load_keys(keys: &LoadKeysResponse) -> String {
    let tag = simulator_tag(keys.terminal_id.as_deref());
    if !keys.is_approved() {
        return format!("{tag}Carga de llaves RECHAZADA — {}", keys.response_code);
    }
    format!(
        "{tag}Llaves cargadas — comercio {} — terminal {}",
        keys.commerce_code.as_deref().unwrap_or("-"),
        keys.terminal_id.as_deref().unwrap_or("-")
    )
}

pub(crate) fn format_close(close: &CloseResponse) -> String {
    let tag = simulator_tag(close.terminal_id.as_deref());
    if !close.is_approved() {
        return format!("{tag}Cierre RECHAZADO — {}", close.response_code);
    }
    format!(
        "{tag}Cierre APROBADO — comercio {} — terminal {}",
        close.commerce_code.as_deref().unwrap_or("-"),
        close.terminal_id.as_deref().unwrap_or("-")
    )
}

pub(crate) fn format_port(port: &PortInfo) -> String {
    let mut text = format!("{}  {:?}", port.name, port.kind);
    if let (Some(vid), Some(pid)) = (port.vid, port.pid) {
        let _ = write!(text, "  {vid:04x}:{pid:04x}");
    }
    if let Some(product) = &port.product {
        text.push_str("  ");
        text.push_str(product);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use transbank_pos::message::{parse_refund, parse_sale, parse_totals};

    const SALE: &str = "0210|00|597029414300|IT750050|abc123|757752|12000|03|3334|9480|000135|CR|22102025|0000000000000000000|VI|22102025|114016|7|500|";

    #[test]
    fn thousands() {
        assert_eq!(clp(0), "$0");
        assert_eq!(clp(999), "$999");
        assert_eq!(clp(1_000), "$1.000");
        assert_eq!(clp(3_334), "$3.334");
        assert_eq!(clp(1_234_567), "$1.234.567");
    }

    #[test]
    fn approved_sale_line() {
        let sale = parse_sale(SALE).unwrap();
        assert_eq!(
            format_sale("Venta", &sale),
            "Venta APROBADA — autorización 757752 — $12.000 — VISA crédito ···9480 — 3 cuotas de $3.334 — operación 000135"
        );
    }

    #[test]
    fn sale_without_installments_and_simulator_tag() {
        let payload = SALE
            .replace("|03|3334|", "|00|0|")
            .replace("IT750050", "SIM00001")
            .replace("|CR|", "|DB|")
            .replace("|VI|", "|DB|");
        let sale = parse_sale(&payload).unwrap();
        assert_eq!(
            format_sale("Venta", &sale),
            "[SIMULADOR] Venta APROBADA — autorización 757752 — $12.000 — Redcompra débito ···9480 — operación 000135"
        );
    }

    #[test]
    fn rejected_sale_line() {
        let sale = parse_sale("0210|01|597029414300|IT750050").unwrap();
        assert_eq!(
            format_sale("Venta", &sale),
            "Venta RECHAZADA — 01 Rechazado"
        );
    }

    #[test]
    fn brands() {
        assert_eq!(brand_name(&CardBrand::Mastercard), "Mastercard");
        assert_eq!(brand_name(&CardBrand::from_code("TV")), "TV");
        assert_eq!(card_type_name(&CardType::Prepaid), "prepago");
    }

    #[test]
    fn refund_and_totals() {
        let refund = parse_refund("1210|00|597029414300|IT750050|162529|000142||").unwrap();
        assert_eq!(
            format_refund(&refund),
            "Anulación APROBADA — autorización 162529 — operación 000142"
        );
        let refund = parse_refund("1210|08|597029414300|IT750050|||").unwrap();
        assert_eq!(
            format_refund(&refund),
            "Anulación RECHAZADA — 08 No puede Anular Transacción Debito"
        );
        let totals = parse_totals("0710|00|002|15000||").unwrap();
        assert_eq!(format_totals(&totals), "2 ventas — total $15.000");
        let totals = parse_totals("0710|00|001|500||").unwrap();
        assert_eq!(format_totals(&totals), "1 venta — total $500");
    }
}
