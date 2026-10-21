//! Typed requests and responses of the POS commands, built on top of [`crate::protocol`].
//!
//! The clients use these functions internally; they are public for advanced users
//! and for the simulator. Each `*_payload` function returns the payload exactly as
//! `docs/protocol-spec.md` §4 defines it, and each `parse_*` function parses one
//! response of §5–§6.

mod integrado;
mod responses;

pub use integrado::{
    MultiCodeSaleRequest, SaleRequest, SalesDetailMode, close_payload, last_sale_payload,
    load_keys_payload, multicode_last_sale_payload, multicode_sale_payload,
    multicode_sales_detail_payload, normal_mode_payload, poll_payload, refund_payload,
    sale_payload, sales_detail_payload, totals_payload,
};
pub use responses::{
    CloseResponse, DetailEntry, LoadKeysResponse, MultiCodeDetailEntry, MultiCodeSaleResponse,
    RefundResponse, SaleResponse, TotalsResponse, is_detail_terminator, parse_close,
    parse_detail_entry, parse_intermediate, parse_load_keys, parse_multicode_detail_entry,
    parse_multicode_sale, parse_refund, parse_sale, parse_totals,
};
