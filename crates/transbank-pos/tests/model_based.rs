//! Model-based test: random sequences of operations through `PosIntegrado` against
//! the simulator must agree with a simple model of the POS business rules
//! (`docs/simulator-spec.md` §3): sales, refunds, totals, detail, last sale, close.

use proptest::prelude::*;
use transbank_pos::transport::ManualClock;
use transbank_pos::{
    LinkConfig, OperationNumber, PosIntegrado, ResponseCode, SaleRequest, SalesDetailMode,
};
use transbank_pos_sim::{PosSimulator, SimTransport, TerminalKind};

#[derive(Debug, Clone)]
enum Op {
    Sale(u32, String),
    Refund(u32),
    Totals,
    Detail,
    LastSale,
    Close,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => (50u32..60_000, "[0-9A-Z]{1,6}").prop_map(|(amount, ticket)| Op::Sale(amount, ticket)),
        2 => (1u32..15).prop_map(Op::Refund),
        1 => Just(Op::Totals),
        1 => Just(Op::Detail),
        1 => Just(Op::LastSale),
        1 => Just(Op::Close),
    ]
}

#[derive(Debug, Clone)]
struct ModelSale {
    amount: u32,
    ticket: String,
    operation: u32,
    refunded: bool,
}

#[derive(Debug, Default)]
struct Model {
    sales: Vec<ModelSale>,
    next_operation: u32,
}

impl Model {
    fn take_operation(&mut self) -> u32 {
        self.next_operation += 1;
        self.next_operation
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn client_and_simulator_agree_with_the_model(ops in prop::collection::vec(op(), 1..40)) {
        let clock = ManualClock::new();
        let sim = SimTransport::new(PosSimulator::new(TerminalKind::Integrado), clock.clone());
        let mut pos = PosIntegrado::with_clock(sim, LinkConfig::default(), clock);
        let mut model = Model::default();

        for op in ops {
            match op {
                Op::Sale(amount, ticket) => {
                    let sale = pos.sale(&SaleRequest::new(amount, &ticket).unwrap()).unwrap();
                    prop_assert!(sale.is_approved());
                    let operation = model.take_operation();
                    prop_assert_eq!(sale.operation_number, Some(format!("{operation:06}")));
                    prop_assert_eq!(sale.amount, Some(amount));
                    let credit = amount % 2 == 0;
                    let expected_installments = if credit && amount >= 10_000 { 3 } else { 0 };
                    prop_assert_eq!(sale.installments_number, Some(expected_installments));
                    model.sales.push(ModelSale { amount, ticket, operation, refunded: false });
                }
                Op::Refund(operation) => {
                    let refund = pos.refund(OperationNumber::new(operation).unwrap()).unwrap();
                    let expected = match model.sales.iter_mut().find(|s| s.operation == operation) {
                        None => ResponseCode::NoTransactionToRefund,
                        Some(sale) if sale.amount % 2 == 1 => ResponseCode::DebitCannotBeRefunded,
                        Some(sale) if sale.refunded => ResponseCode::AlreadyRefunded,
                        Some(sale) => {
                            sale.refunded = true;
                            ResponseCode::Approved
                        }
                    };
                    if expected == ResponseCode::Approved {
                        let refund_operation = model.take_operation();
                        prop_assert_eq!(refund.operation_number, Some(format!("{refund_operation:06}")));
                    }
                    prop_assert_eq!(refund.response_code, expected);
                }
                Op::Totals => {
                    let totals = pos.totals().unwrap();
                    let active: Vec<_> = model.sales.iter().filter(|s| !s.refunded).collect();
                    prop_assert_eq!(totals.tx_count, Some(u32::try_from(active.len()).unwrap()));
                    prop_assert_eq!(totals.tx_total, Some(active.iter().map(|s| u64::from(s.amount)).sum()));
                }
                Op::Detail => {
                    let detail = pos.sales_detail(SalesDetailMode::SendToHost).unwrap();
                    let tickets: Vec<_> = detail.iter().map(|e| e.ticket.clone().unwrap_or_default()).collect();
                    let expected: Vec<_> = model.sales.iter().map(|s| format!("{:0>6}", s.ticket)).collect();
                    prop_assert_eq!(tickets, expected);
                }
                Op::LastSale => {
                    let last = pos.last_sale().unwrap();
                    match model.sales.last() {
                        None => prop_assert_eq!(last.response_code, ResponseCode::NoSale),
                        Some(sale) => {
                            prop_assert!(last.is_approved());
                            prop_assert_eq!(last.amount, Some(sale.amount));
                            prop_assert_eq!(last.operation_number, Some(format!("{:06}", sale.operation)));
                        }
                    }
                }
                Op::Close => {
                    prop_assert!(pos.close().unwrap().is_approved());
                    model.sales.clear();
                }
            }
        }
    }
}
