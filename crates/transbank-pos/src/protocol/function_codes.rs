//! Function codes (field 0) of the POS Integrado commands and responses
//! (`docs/protocol-spec.md` §4–§6).

/// Poll (`0100`), answered only with ACK.
pub const FC_POLL: &str = "0100";
/// Switch to normal mode (`0300`), answered only with ACK.
pub const FC_NORMAL_MODE: &str = "0300";
/// Load keys request.
pub const FC_LOAD_KEYS: &str = "0800";
/// Load keys response.
pub const FC_LOAD_KEYS_RESPONSE: &str = "0810";
/// Sale request.
pub const FC_SALE: &str = "0200";
/// Sale response.
pub const FC_SALE_RESPONSE: &str = "0210";
/// Multi-code sale request.
pub const FC_MULTICODE_SALE: &str = "0270";
/// Multi-code sale response.
pub const FC_MULTICODE_SALE_RESPONSE: &str = "0271";
/// Last sale request.
pub const FC_LAST_SALE: &str = "0250";
/// Last sale response (same layout as the sale response).
pub const FC_LAST_SALE_RESPONSE: &str = "0260";
/// Multi-code last sale request.
pub const FC_MULTICODE_LAST_SALE: &str = "0280";
/// Multi-code last sale response.
pub const FC_MULTICODE_LAST_SALE_RESPONSE: &str = "0281";
/// Refund request.
pub const FC_REFUND: &str = "1200";
/// Refund response.
pub const FC_REFUND_RESPONSE: &str = "1210";
/// Totals request.
pub const FC_TOTALS: &str = "0700";
/// Totals response.
pub const FC_TOTALS_RESPONSE: &str = "0710";
/// Sales detail request (same digits as [`FC_LAST_SALE_RESPONSE`]).
pub const FC_SALES_DETAIL: &str = "0260";
/// Sales detail entry response.
pub const FC_SALES_DETAIL_RESPONSE: &str = "0261";
/// Multi-code sales detail request.
pub const FC_MULTICODE_SALES_DETAIL: &str = "0290";
/// Multi-code sales detail entry response.
pub const FC_MULTICODE_SALES_DETAIL_RESPONSE: &str = "0291";
/// Close request.
pub const FC_CLOSE: &str = "0500";
/// Close response.
pub const FC_CLOSE_RESPONSE: &str = "0510";
/// Intermediate status message.
pub const FC_INTERMEDIATE: &str = "0900";
