//! [`PosIntegrado`]: typed client of the POS Integrado (`docs/protocol-spec.md` §4).

use crate::codes::IntermediateStatus;
use crate::error::Result;
use crate::link::{Link, LinkConfig};
use crate::message::{
    CloseResponse, DetailEntry, LoadKeysResponse, MultiCodeDetailEntry, MultiCodeSaleRequest,
    MultiCodeSaleResponse, RefundResponse, SaleRequest, SaleResponse, SalesDetailMode,
    TotalsResponse, close_payload, last_sale_payload, load_keys_payload,
    multicode_last_sale_payload, multicode_sale_payload, multicode_sales_detail_payload,
    normal_mode_payload, parse_close, parse_detail_entry, parse_load_keys,
    parse_multicode_detail_entry, parse_multicode_sale, parse_refund, parse_sale, parse_totals,
    poll_payload, refund_payload, sale_payload, sales_detail_payload, totals_payload,
};
use crate::protocol::types::OperationNumber;
use crate::protocol::{
    FC_CLOSE_RESPONSE, FC_LAST_SALE_RESPONSE, FC_LOAD_KEYS_RESPONSE,
    FC_MULTICODE_LAST_SALE_RESPONSE, FC_MULTICODE_SALE_RESPONSE,
    FC_MULTICODE_SALES_DETAIL_RESPONSE, FC_REFUND_RESPONSE, FC_SALE_RESPONSE,
    FC_SALES_DETAIL_RESPONSE, FC_TOTALS_RESPONSE,
};
use crate::transport::{Clock, SystemClock, Transport};

fn ignore_status(_: IntermediateStatus) {}

/// Client of a POS Integrado.
///
/// Every operation validates its arguments, sends the command through the
/// [`Link`] and parses the typed response. A declined card is an `Ok` response whose
/// `is_approved()` is `false`; `Err` means a communication or protocol failure
/// (ADR-03). One operation at a time: every method takes `&mut self`.
///
/// ```
/// use transbank_pos::{LinkConfig, PosIntegrado, SaleRequest};
/// use transbank_pos::transport::ManualClock;
/// use transbank_pos_sim::{PosSimulator, SimTransport, TerminalKind};
///
/// let clock = ManualClock::new();
/// let transport = SimTransport::new(PosSimulator::new(TerminalKind::Integrado), clock.clone());
/// let mut pos = PosIntegrado::with_clock(transport, LinkConfig::default(), clock);
///
/// pos.poll()?;
/// let sale = pos.sale(&SaleRequest::new(12_000, "ABC123")?)?;
/// if sale.is_approved() {
///     assert_eq!(sale.amount, Some(12_000));
/// }
/// # Ok::<(), transbank_pos::Error>(())
/// ```
pub struct PosIntegrado<T: Transport, C: Clock = SystemClock> {
    link: Link<T, C>,
}

impl<T: Transport> PosIntegrado<T> {
    /// Creates a client with the default [`LinkConfig`] (2 s ACK, 150 s response).
    #[must_use]
    pub fn new(transport: T) -> Self {
        Self::with_config(transport, LinkConfig::default())
    }

    /// Creates a client with a custom [`LinkConfig`].
    #[must_use]
    pub fn with_config(transport: T, config: LinkConfig) -> Self {
        Self {
            link: Link::new(transport, config),
        }
    }
}

impl<T: Transport, C: Clock> PosIntegrado<T, C> {
    /// Creates a client with an explicit clock (tests use a
    /// [`ManualClock`](crate::transport::ManualClock)).
    #[must_use]
    pub fn with_clock(transport: T, config: LinkConfig, clock: C) -> Self {
        Self {
            link: Link::with_clock(transport, config, clock),
        }
    }

    /// Checks that the POS is connected and in integrated mode (`0100`, ACK only).
    ///
    /// # Errors
    ///
    /// [`Error::NoAck`](crate::Error::NoAck) if the POS does not acknowledge, or a
    /// transport error.
    pub fn poll(&mut self) -> Result<()> {
        self.link.send_command(&poll_payload())
    }

    /// Loads the keys from Transbank (`0800`); recommended at the start of the day
    /// and when a response asks for it (code 26).
    ///
    /// # Errors
    ///
    /// Communication and protocol errors of [`Link::request`].
    pub fn load_keys(&mut self) -> Result<LoadKeysResponse> {
        let response = self.link.request(
            &load_keys_payload(),
            FC_LOAD_KEYS_RESPONSE,
            &mut ignore_status,
        )?;
        parse_load_keys(&response)
    }

    /// Sale (`0200`), without intermediate messages.
    ///
    /// # Timeouts y doble cobro
    ///
    /// If the call fails with [`Error::ResponseTimeout`](crate::Error::ResponseTimeout)
    /// or an I/O error after the POS acknowledged the sale, **the result is unknown**:
    /// the customer may have paid. The client never resends a sale by itself (ADR-04).
    /// Query [`last_sale`](Self::last_sale) and compare ticket and amount before
    /// retrying:
    ///
    /// ```
    /// use transbank_pos::{Error, LinkConfig, PosIntegrado, SaleRequest};
    /// use transbank_pos::transport::ManualClock;
    /// use transbank_pos_sim::{PosSimulator, Scenario, SimTransport, TerminalKind};
    ///
    /// let clock = ManualClock::new();
    /// let sim = PosSimulator::new(TerminalKind::Integrado).with_scenario(Scenario::NoResponse);
    /// let mut pos = PosIntegrado::with_clock(SimTransport::new(sim, clock.clone()), LinkConfig::default(), clock);
    /// let request = SaleRequest::new(12_000, "ABC123")?;
    ///
    /// match pos.sale(&request) {
    ///     Err(Error::ResponseTimeout { .. }) => {
    ///         pos.link_mut().transport_mut().sim_mut().set_scenario(Scenario::Approve);
    ///         let last = pos.last_sale()?;
    ///         let charged = last.is_approved()
    ///             && last.ticket.as_deref() == Some(request.ticket().as_str())
    ///             && last.amount == Some(request.amount().get());
    ///         assert!(charged, "the sale went through: do not charge again");
    ///     }
    ///     other => panic!("{other:?}"),
    /// }
    /// # Ok::<(), transbank_pos::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Communication and protocol errors of [`Link::request`]. A declined card is
    /// `Ok` with `is_approved() == false`.
    pub fn sale(&mut self, req: &SaleRequest) -> Result<SaleResponse> {
        let response = self.link.request(
            &sale_payload(req, false),
            FC_SALE_RESPONSE,
            &mut ignore_status,
        )?;
        parse_sale(&response)
    }

    /// Sale (`0200`) asking the POS for intermediate messages (`0900`), delivered to
    /// `on_status` while the customer operates the POS. See [`sale`](Self::sale)
    /// about timeouts.
    ///
    /// # Errors
    ///
    /// Same as [`sale`](Self::sale).
    pub fn sale_with_status(
        &mut self,
        req: &SaleRequest,
        mut on_status: impl FnMut(IntermediateStatus),
    ) -> Result<SaleResponse> {
        let response =
            self.link
                .request(&sale_payload(req, true), FC_SALE_RESPONSE, &mut on_status)?;
        parse_sale(&response)
    }

    /// Multi-code sale (`0270`) for a child commerce. See [`sale`](Self::sale) about
    /// timeouts (use [`multicode_last_sale`](Self::multicode_last_sale)).
    ///
    /// # Errors
    ///
    /// Same as [`sale`](Self::sale).
    pub fn multicode_sale(&mut self, req: &MultiCodeSaleRequest) -> Result<MultiCodeSaleResponse> {
        self.request_multicode_sale(req, false, &mut ignore_status)
    }

    /// Multi-code sale (`0270`) with intermediate messages.
    ///
    /// # Errors
    ///
    /// Same as [`sale`](Self::sale).
    pub fn multicode_sale_with_status(
        &mut self,
        req: &MultiCodeSaleRequest,
        mut on_status: impl FnMut(IntermediateStatus),
    ) -> Result<MultiCodeSaleResponse> {
        self.request_multicode_sale(req, true, &mut on_status)
    }

    fn request_multicode_sale(
        &mut self,
        req: &MultiCodeSaleRequest,
        status: bool,
        on_status: &mut dyn FnMut(IntermediateStatus),
    ) -> Result<MultiCodeSaleResponse> {
        let response = self.link.request(
            &multicode_sale_payload(req, status),
            FC_MULTICODE_SALE_RESPONSE,
            on_status,
        )?;
        parse_multicode_sale(&response)
    }

    /// Last sale registered by the POS (`0250`); the way to resolve a sale whose
    /// result is unknown. Code 11 means there is no sale.
    ///
    /// # Errors
    ///
    /// Communication and protocol errors of [`Link::request`].
    pub fn last_sale(&mut self) -> Result<SaleResponse> {
        let response = self.link.request(
            &last_sale_payload(),
            FC_LAST_SALE_RESPONSE,
            &mut ignore_status,
        )?;
        parse_sale(&response)
    }

    /// Last multi-code sale (`0280`), optionally with its voucher.
    ///
    /// # Errors
    ///
    /// Communication and protocol errors of [`Link::request`].
    pub fn multicode_last_sale(&mut self, voucher: bool) -> Result<MultiCodeSaleResponse> {
        let response = self.link.request(
            &multicode_last_sale_payload(voucher),
            FC_MULTICODE_LAST_SALE_RESPONSE,
            &mut ignore_status,
        )?;
        parse_multicode_sale(&response)
    }

    /// Refunds a credit sale by its operation number (`1200`). Debit sales cannot be
    /// refunded (code 8). Never retried automatically (ADR-04).
    ///
    /// # Errors
    ///
    /// Communication and protocol errors of [`Link::request`].
    pub fn refund(&mut self, operation: OperationNumber) -> Result<RefundResponse> {
        let response = self.link.request(
            &refund_payload(operation),
            FC_REFUND_RESPONSE,
            &mut ignore_status,
        )?;
        parse_refund(&response)
    }

    /// Number and amount of the sales since the last close (`0700`).
    ///
    /// # Errors
    ///
    /// Communication and protocol errors of [`Link::request`].
    pub fn totals(&mut self) -> Result<TotalsResponse> {
        let response =
            self.link
                .request(&totals_payload(), FC_TOTALS_RESPONSE, &mut ignore_status)?;
        parse_totals(&response)
    }

    /// Sales detail (`0260`). [`SalesDetailMode::PrintOnPos`] prints it on the POS
    /// and returns an empty list; [`SalesDetailMode::SendToHost`] returns one entry
    /// per sale.
    ///
    /// # Errors
    ///
    /// Communication and protocol errors of [`Link::request_list`].
    pub fn sales_detail(&mut self, mode: SalesDetailMode) -> Result<Vec<DetailEntry>> {
        let payload = sales_detail_payload(mode);
        match mode {
            SalesDetailMode::PrintOnPos => self.link.send_command(&payload).map(|()| Vec::new()),
            SalesDetailMode::SendToHost => self
                .link
                .request_list(&payload, FC_SALES_DETAIL_RESPONSE)?
                .iter()
                .map(|entry| parse_detail_entry(entry))
                .collect(),
        }
    }

    /// Multi-code sales detail (`0290`); see [`sales_detail`](Self::sales_detail).
    ///
    /// # Errors
    ///
    /// Communication and protocol errors of [`Link::request_list`].
    pub fn multicode_sales_detail(
        &mut self,
        mode: SalesDetailMode,
    ) -> Result<Vec<MultiCodeDetailEntry>> {
        let payload = multicode_sales_detail_payload(mode);
        match mode {
            SalesDetailMode::PrintOnPos => self.link.send_command(&payload).map(|()| Vec::new()),
            SalesDetailMode::SendToHost => self
                .link
                .request_list(&payload, FC_MULTICODE_SALES_DETAIL_RESPONSE)?
                .iter()
                .map(|entry| parse_multicode_detail_entry(entry))
                .collect(),
        }
    }

    /// Day close (`0500`): the POS prints the report and starts a new period.
    ///
    /// # Errors
    ///
    /// Communication and protocol errors of [`Link::request`].
    pub fn close(&mut self) -> Result<CloseResponse> {
        let response =
            self.link
                .request(&close_payload(), FC_CLOSE_RESPONSE, &mut ignore_status)?;
        parse_close(&response)
    }

    /// Takes the POS out of integrated mode (`0300`, ACK only). It has to be
    /// re-enabled from the POS menu.
    ///
    /// # Errors
    ///
    /// [`Error::NoAck`](crate::Error::NoAck) or a transport error.
    pub fn set_normal_mode(&mut self) -> Result<()> {
        self.link.send_command(&normal_mode_payload())
    }

    /// The underlying link (configuration, transport).
    pub fn link_mut(&mut self) -> &mut Link<T, C> {
        &mut self.link
    }

    /// Returns the transport.
    #[must_use]
    pub fn into_inner(self) -> T {
        self.link.into_inner()
    }
}
