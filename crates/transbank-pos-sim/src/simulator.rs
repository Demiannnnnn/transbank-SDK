//! Sans-IO POS simulator (`docs/simulator-spec.md` §1, §3).

use std::collections::VecDeque;
use std::mem;

use transbank_pos::protocol::{ACK, FrameDecoder, LinkEvent, NAK, decode_latin1, encode_frame};
use transbank_pos::{CardBrand, CardType, PosDate, PosTime};

use crate::request::{SaleArgs, SimRequest, parse_request};
use crate::scenario::Scenario;

/// Commerce code reported by the simulator (Transbank test commerce).
pub const SIM_COMMERCE_CODE: &str = "597029414300";
/// Terminal id reported by the simulator; clients show "SIMULADOR" for `SIM*`.
pub const SIM_TERMINAL_ID: &str = "SIM00001";

/// Retransmissions of a response answered with NAK.
const MAX_RETRANSMISSIONS: u8 = 3;
/// Intermediate codes sent by default when the host asks for them.
const DEFAULT_INTERMEDIATE: [u16; 4] = [78, 79, 81, 82];
/// Lines of the voucher when the host asks for it without [`Scenario::WithVoucher`].
const DEFAULT_VOUCHER_LINES: u8 = 3;
/// Width of a voucher line.
const VOUCHER_LINE: usize = 40;
/// Credit sales from this amount are paid in installments.
const INSTALLMENTS_FROM: u64 = 10_000;
/// Number of installments of a credit sale of at least [`INSTALLMENTS_FROM`].
const INSTALLMENTS: u64 = 3;

/// Model of terminal simulated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum TerminalKind {
    /// POS Integrado.
    #[default]
    Integrado,
    /// POS Autoservicio. Pending (AUTO-01): answers ACK to `0100` and response
    /// code 12 ("Transacción No Soportada") to everything else.
    Autoservicio,
}

/// A sale registered by the simulator since the last close.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SimulatedSale {
    /// Amount in CLP.
    pub amount: u64,
    /// Ticket as received.
    pub ticket: String,
    /// Authorization code.
    pub authorization_code: String,
    /// Operation number.
    pub operation_number: u32,
    /// Card type (credit for even amounts, debit for odd ones).
    pub card_type: CardType,
    /// Card brand.
    pub card_brand: CardBrand,
    /// Number of installments (0 if none).
    pub installments: u64,
    /// The sale was refunded.
    pub refunded: bool,
    /// Commerce code of a multi-code sale.
    pub commerce_code: Option<String>,
    last4: String,
}

/// Response waiting for the host ACK.
#[derive(Debug)]
struct Awaiting {
    frame: Vec<u8>,
    retransmissions: u8,
}

/// A fake POS that speaks the protocol from the terminal side, without I/O.
///
/// Feed it the bytes written by the host; it returns the bytes the POS sends right
/// away (ACK/NAK and the next response frame). After sending a frame it waits for
/// the host ACK before sending the next one, and retransmits it on NAK (up to 3
/// times).
///
/// ```
/// use transbank_pos::protocol::encode_frame;
/// use transbank_pos_sim::{PosSimulator, TerminalKind};
///
/// let mut sim = PosSimulator::new(TerminalKind::Integrado);
/// assert_eq!(sim.feed(&encode_frame("0100")?), [0x06]);
/// assert_eq!(sim.received_payloads(), ["0100"]);
/// # Ok::<(), transbank_pos::Error>(())
/// ```
#[derive(Debug)]
pub struct PosSimulator {
    kind: TerminalKind,
    scenario: Scenario,
    decoder: FrameDecoder,
    events: Vec<LinkEvent>,
    outbox: VecDeque<String>,
    awaiting: Option<Awaiting>,
    sales: Vec<SimulatedSale>,
    received: Vec<String>,
    next_operation: u32,
    date: PosDate,
    time: PosTime,
}

impl PosSimulator {
    /// Creates a simulator that approves everything, dated 01-01-2026 12:00:00.
    #[must_use]
    pub fn new(kind: TerminalKind) -> Self {
        Self {
            kind,
            scenario: Scenario::Approve,
            decoder: FrameDecoder::new(),
            events: Vec::new(),
            outbox: VecDeque::new(),
            awaiting: None,
            sales: Vec::new(),
            received: Vec::new(),
            next_operation: 1,
            date: PosDate {
                day: 1,
                month: 1,
                year: 2026,
            },
            time: PosTime {
                hour: 12,
                minute: 0,
                second: 0,
            },
        }
    }

    /// Sets the scenario.
    #[must_use]
    pub fn with_scenario(mut self, scenario: Scenario) -> Self {
        self.scenario = scenario;
        self
    }

    /// Sets the fixed date and time reported in every response.
    #[must_use]
    pub fn with_clock_fixed(mut self, date: PosDate, time: PosTime) -> Self {
        self.date = date;
        self.time = time;
        self
    }

    /// Replaces the scenario.
    pub fn set_scenario(&mut self, scenario: Scenario) {
        self.scenario = scenario;
    }

    /// Current scenario (a consumed "next N" scenario is back to `Approve`).
    #[must_use]
    pub fn scenario(&self) -> &Scenario {
        &self.scenario
    }

    /// Kind of terminal simulated.
    #[must_use]
    pub fn kind(&self) -> TerminalKind {
        self.kind
    }

    /// Sales registered since the last close.
    #[must_use]
    pub fn sales(&self) -> &[SimulatedSale] {
        &self.sales
    }

    /// Every valid frame received from the host, decoded (for asserts).
    #[must_use]
    pub fn received_payloads(&self) -> &[String] {
        &self.received
    }

    /// Feeds bytes written by the host and returns the bytes the POS sends now.
    pub fn feed(&mut self, input: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        self.decoder.push(input, &mut self.events);
        for event in mem::take(&mut self.events) {
            match event {
                LinkEvent::Frame(payload) => self.on_command(&decode_latin1(&payload), &mut out),
                LinkEvent::BadLrc { .. } => out.push(NAK),
                LinkEvent::Ack => {
                    if self.awaiting.take().is_some() {
                        self.send_next(&mut out);
                    }
                }
                LinkEvent::Nak => self.retransmit(&mut out),
                _ => {}
            }
        }
        out
    }

    fn on_command(&mut self, payload: &str, out: &mut Vec<u8>) {
        self.received.push(payload.to_owned());
        if let Scenario::DropAcks(n) = self.scenario {
            if n > 0 {
                self.scenario = countdown(n, Scenario::DropAcks);
                return;
            }
        }
        if let Scenario::NakNext(n) = self.scenario {
            if n > 0 {
                self.scenario = countdown(n, Scenario::NakNext);
                out.push(NAK);
                return;
            }
        }
        // A new command supersedes any response still waiting for its ACK.
        self.awaiting = None;
        self.outbox.clear();
        out.push(ACK);
        let mut responses = match self.kind {
            TerminalKind::Integrado => self.respond(payload),
            TerminalKind::Autoservicio => unsupported(payload),
        };
        if responses.is_empty() || self.scenario == Scenario::NoResponse {
            return;
        }
        if let Scenario::WrongFunctionCode(code) = self.scenario {
            if let Some(response) = responses
                .iter_mut()
                .find(|response| !response.starts_with("0900|"))
            {
                *response = replace_function_code(response, code);
            }
            self.scenario = Scenario::Approve;
        }
        if matches!(self.scenario, Scenario::Garbage(_)) {
            if let Scenario::Garbage(garbage) = mem::take(&mut self.scenario) {
                out.extend(garbage);
            }
        }
        self.outbox.extend(responses);
        self.send_next(out);
    }

    fn send_next(&mut self, out: &mut Vec<u8>) {
        if self.awaiting.is_some() {
            return;
        }
        while let Some(payload) = self.outbox.pop_front() {
            // Responses are built from ASCII fields; an unencodable one is skipped.
            if let Ok(frame) = encode_frame(&payload) {
                self.transmit(&frame, out);
                self.awaiting = Some(Awaiting {
                    frame,
                    retransmissions: 0,
                });
                return;
            }
        }
    }

    fn retransmit(&mut self, out: &mut Vec<u8>) {
        let Some(awaiting) = self.awaiting.as_mut() else {
            return;
        };
        if awaiting.retransmissions >= MAX_RETRANSMISSIONS {
            self.awaiting = None;
            self.outbox.clear();
            return;
        }
        awaiting.retransmissions += 1;
        let frame = awaiting.frame.clone();
        self.transmit(&frame, out);
    }

    fn transmit(&mut self, frame: &[u8], out: &mut Vec<u8>) {
        if let Scenario::CorruptLrc(n) = self.scenario {
            if n > 0 {
                self.scenario = countdown(n, Scenario::CorruptLrc);
                let start = out.len();
                out.extend_from_slice(frame);
                if let Some(lrc) = out.get_mut(start..).and_then(<[u8]>::last_mut) {
                    *lrc ^= 0xFF;
                }
                return;
            }
        }
        out.extend_from_slice(frame);
    }

    /// Responses of the POS Integrado (`docs/simulator-spec.md` §3).
    fn respond(&mut self, payload: &str) -> Vec<String> {
        let Ok(request) = parse_request(payload) else {
            return vec![format!("{}|61", response_code_for(payload))];
        };
        match request {
            SimRequest::LoadKeys => vec![format!("0810|00|{SIM_COMMERCE_CODE}|{SIM_TERMINAL_ID}")],
            SimRequest::Sale(sale) => self.sale(&sale, None),
            SimRequest::MultiCodeSale {
                sale,
                commerce_code,
            } => self.sale(&sale, Some(commerce_code)),
            SimRequest::LastSale => vec![self.last_sale("0260", None)],
            SimRequest::MultiCodeLastSale { voucher } => {
                vec![self.last_sale("0281", Some(voucher))]
            }
            SimRequest::Refund { operation } => vec![self.refund(operation)],
            SimRequest::Totals => vec![self.totals()],
            SimRequest::SalesDetail { send_to_host: true } => self.detail("0261"),
            SimRequest::MultiCodeSalesDetail { send_to_host: true } => self.detail("0291"),
            SimRequest::Close => {
                self.sales.clear();
                if self.scenario == Scenario::RequireClose {
                    self.scenario = Scenario::Approve;
                }
                vec![format!("0510|00|{SIM_COMMERCE_CODE}|{SIM_TERMINAL_ID}|")]
            }
            _ => Vec::new(),
        }
    }

    fn sale(&mut self, args: &SaleArgs, commerce_code: Option<String>) -> Vec<String> {
        let function_code = if commerce_code.is_some() {
            "0271"
        } else {
            "0210"
        };
        let rejection = if args.amount < 50 {
            Some(10)
        } else if args.amount > 999_999_999 {
            Some(68)
        } else if args.ticket.len() > 6 {
            Some(70)
        } else if self.scenario == Scenario::RequireClose {
            Some(74)
        } else {
            None
        };
        if let Some(code) = rejection {
            return vec![rejected(function_code, code, args)];
        }
        let mut frames: Vec<String> = if args.status {
            let codes = match &self.scenario {
                Scenario::Intermediate(codes) => codes.clone(),
                _ => DEFAULT_INTERMEDIATE.to_vec(),
            };
            codes.iter().map(|code| format!("0900|{code}")).collect()
        } else {
            Vec::new()
        };
        if let Scenario::RejectNextSale(code) = self.scenario {
            self.scenario = Scenario::Approve;
            frames.push(rejected(function_code, code, args));
            return frames;
        }
        let sale = self.register_sale(args, commerce_code);
        let voucher_lines = match self.scenario {
            Scenario::WithVoucher(lines) => lines,
            _ if args.voucher => DEFAULT_VOUCHER_LINES,
            _ => 0,
        };
        let mut fields = self.sale_fields(function_code, &sale);
        fields.push(voucher(&sale, voucher_lines));
        if let Some(code) = &sale.commerce_code {
            fields.push("0".to_owned());
            fields.push(provider(code));
        }
        self.sales.push(sale);
        frames.push(fields.join("|"));
        frames
    }

    fn register_sale(&mut self, args: &SaleArgs, commerce_code: Option<String>) -> SimulatedSale {
        let operation_number = self.take_operation();
        let credit = args.amount % 2 == 0;
        SimulatedSale {
            amount: args.amount,
            ticket: args.ticket.clone(),
            authorization_code: authorization(operation_number),
            operation_number,
            card_type: if credit {
                CardType::Credit
            } else {
                CardType::Debit
            },
            card_brand: if credit {
                CardBrand::Visa
            } else {
                CardBrand::Redcompra
            },
            installments: if credit && args.amount >= INSTALLMENTS_FROM {
                INSTALLMENTS
            } else {
                0
            },
            refunded: false,
            commerce_code,
            last4: format!("{:04}", (operation_number * 37) % 10_000),
        }
    }

    fn take_operation(&mut self) -> u32 {
        let operation = self.next_operation;
        self.next_operation = if operation >= 999_999 {
            1
        } else {
            operation + 1
        };
        operation
    }

    /// Fields 0–18 with the `0210` layout.
    fn sale_fields(&self, function_code: &str, sale: &SimulatedSale) -> Vec<String> {
        let installment_amount = if sale.installments > 0 {
            sale.amount.div_ceil(sale.installments)
        } else {
            0
        };
        vec![
            function_code.to_owned(),
            "00".to_owned(),
            SIM_COMMERCE_CODE.to_owned(),
            SIM_TERMINAL_ID.to_owned(),
            sale.ticket.clone(),
            sale.authorization_code.clone(),
            sale.amount.to_string(),
            format!("{:02}", sale.installments),
            installment_amount.to_string(),
            sale.last4.clone(),
            format!("{:06}", sale.operation_number),
            sale.card_type.code().to_owned(),
            self.date_text(),
            format!("{:0>19}", sale.last4),
            sale.card_brand.code().to_owned(),
            self.date_text(),
            self.time_text(),
            String::new(),
            "0".to_owned(),
        ]
    }

    fn last_sale(&self, function_code: &str, voucher_flag: Option<bool>) -> String {
        let Some(sale) = self.sales.last() else {
            return format!("{function_code}|11|{SIM_COMMERCE_CODE}|{SIM_TERMINAL_ID}");
        };
        let mut fields = self.sale_fields(function_code, sale);
        if let Some(wants_voucher) = voucher_flag {
            let lines = if wants_voucher {
                DEFAULT_VOUCHER_LINES
            } else {
                0
            };
            fields.push(voucher(sale, lines));
            fields.push("0".to_owned());
            fields.push(
                sale.commerce_code
                    .as_deref()
                    .map(provider)
                    .unwrap_or_default(),
            );
        } else {
            fields.push(String::new());
        }
        fields.join("|")
    }

    fn refund(&mut self, operation: u32) -> String {
        let code = match self
            .sales
            .iter()
            .position(|sale| sale.operation_number == operation)
        {
            None => 5,
            Some(index) => match self.sales.get(index) {
                Some(sale) if sale.card_type != CardType::Credit => 8,
                Some(sale) if sale.refunded => 4,
                _ => {
                    if let Some(sale) = self.sales.get_mut(index) {
                        sale.refunded = true;
                    }
                    let refund_operation = self.take_operation();
                    return format!(
                        "1210|00|{SIM_COMMERCE_CODE}|{SIM_TERMINAL_ID}|{}|{refund_operation:06}|",
                        authorization(refund_operation)
                    );
                }
            },
        };
        format!("1210|{code:02}|{SIM_COMMERCE_CODE}|{SIM_TERMINAL_ID}|||")
    }

    fn totals(&self) -> String {
        let active = self.sales.iter().filter(|sale| !sale.refunded);
        let (count, total) = active.fold((0u64, 0u64), |(count, total), sale| {
            (count + 1, total + sale.amount)
        });
        format!("0710|00|{count:03}|{total}||")
    }

    fn detail(&self, function_code: &str) -> Vec<String> {
        if self.sales.is_empty() {
            return vec![format!("{function_code}|11||||||||||||||||||")];
        }
        let mut frames: Vec<String> = self
            .sales
            .iter()
            .map(|sale| {
                if function_code == "0261" {
                    self.detail_fields(sale).join("|")
                } else {
                    let mut fields = self.sale_fields(function_code, sale);
                    fields.push("0".to_owned());
                    fields.push(
                        sale.commerce_code
                            .as_deref()
                            .map(provider)
                            .unwrap_or_default(),
                    );
                    fields.join("|")
                }
            })
            .collect();
        let terminator =
            format!("{function_code}|00|{SIM_COMMERCE_CODE}|{SIM_TERMINAL_ID}||||||||||||||||");
        frames.push(terminator.clone());
        frames.push(terminator);
        frames
    }

    /// `0261` layout (`docs/protocol-spec.md` §5.3).
    fn detail_fields(&self, sale: &SimulatedSale) -> Vec<String> {
        let fields = self.sale_fields("0261", sale);
        let at = |index: usize| fields.get(index).cloned().unwrap_or_default();
        vec![
            at(0),
            at(1),
            at(2),
            at(3),
            at(4),
            at(5),
            at(6),
            at(9),
            at(10),
            at(11),
            at(12),
            at(13),
            at(14),
            at(15),
            at(16),
            at(17),
            at(18),
            at(8),
            at(7),
        ]
    }

    fn date_text(&self) -> String {
        format!(
            "{:02}{:02}{:04}",
            self.date.day, self.date.month, self.date.year
        )
    }

    fn time_text(&self) -> String {
        format!(
            "{:02}{:02}{:02}",
            self.time.hour, self.time.minute, self.time.second
        )
    }
}

fn countdown(n: u8, scenario: fn(u8) -> Scenario) -> Scenario {
    if n > 1 {
        scenario(n - 1)
    } else {
        Scenario::Approve
    }
}

/// Deterministic authorization code derived from the operation number (no `rand`).
fn authorization(operation: u32) -> String {
    let code = (u64::from(operation) * 7_919 + 104_729) % 1_000_000;
    format!("{code:06}")
}

fn provider(commerce_code: &str) -> String {
    if commerce_code == "0" {
        SIM_COMMERCE_CODE.to_owned()
    } else {
        commerce_code.to_owned()
    }
}

fn voucher(sale: &SimulatedSale, lines: u8) -> String {
    (1..=lines)
        .map(|line| {
            let text = format!(
                "SIMULADOR {line:02} TICKET {} ${}",
                sale.ticket, sale.amount
            );
            let mut text: String = text.chars().take(VOUCHER_LINE).collect();
            while text.len() < VOUCHER_LINE {
                text.push(' ');
            }
            text
        })
        .collect()
}

fn rejected(function_code: &str, code: u16, args: &SaleArgs) -> String {
    format!(
        "{function_code}|{code:02}|{SIM_COMMERCE_CODE}|{SIM_TERMINAL_ID}|{}||{}|",
        args.ticket, args.amount
    )
}

/// Response function code of a request: request code + 10 (`0200` ⇒ `0210`).
fn response_code_for(payload: &str) -> String {
    let request = payload.split('|').next().unwrap_or_default().trim();
    request
        .parse::<u16>()
        .ok()
        .filter(|_| request.len() == 4)
        .map_or_else(
            || "0000".to_owned(),
            |code| format!("{:04}", code.saturating_add(10)),
        )
}

fn unsupported(payload: &str) -> Vec<String> {
    if payload.split('|').next() == Some("0100") {
        Vec::new()
    } else {
        vec![format!("{}|12", response_code_for(payload))]
    }
}

fn replace_function_code(payload: &str, code: &str) -> String {
    match payload.split_once('|') {
        Some((_, rest)) => format!("{code}|{rest}"),
        None => code.to_owned(),
    }
}
