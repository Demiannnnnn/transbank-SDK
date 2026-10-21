//! `--json` output with the same redaction as `Debug` (security-model §5).

use serde_json::Value;

fn redacted(value: &str) -> Value {
    Value::String(format!("<redacted {} chars>", value.chars().count()))
}

/// Replaces `account_number`, `raw` and (unless `show_voucher`) `voucher` by their
/// length, at any depth.
pub(crate) fn redact(value: &mut Value, show_voucher: bool) {
    match value {
        Value::Object(map) => {
            for (key, field) in map.iter_mut() {
                let sensitive = match key.as_str() {
                    "account_number" | "raw" => true,
                    "voucher" => !show_voucher,
                    _ => false,
                };
                match field {
                    Value::String(text) if sensitive => *field = redacted(text),
                    _ => redact(field, show_voucher),
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                redact(item, show_voucher);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn redacts_nested_fields() {
        let mut value = json!({
            "sale": {"account_number": "123456", "voucher": "LINE", "raw": "0210|00", "ticket": "1"},
            "list": [{"account_number": "99"}],
            "voucher": null
        });
        redact(&mut value, false);
        assert_eq!(value["sale"]["account_number"], "<redacted 6 chars>");
        assert_eq!(value["sale"]["voucher"], "<redacted 4 chars>");
        assert_eq!(value["sale"]["raw"], "<redacted 7 chars>");
        assert_eq!(value["sale"]["ticket"], "1");
        assert_eq!(value["list"][0]["account_number"], "<redacted 2 chars>");
        assert_eq!(value["voucher"], Value::Null);
    }

    #[test]
    fn show_voucher_keeps_only_the_voucher() {
        let mut value = json!({"voucher": "LINE", "account_number": "123"});
        redact(&mut value, true);
        assert_eq!(value["voucher"], "LINE");
        assert_eq!(value["account_number"], "<redacted 3 chars>");
    }
}
