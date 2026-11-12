//! [`PosAutoservicio`]: typed client of the POS Autoservicio (`docs/protocol-spec.md` §7).

use crate::codes::IntermediateStatus;
use crate::error::Result;
use crate::link::{Link, LinkConfig, TraceLayout};
use crate::message::{
    CloseResponse, InitializationResponse, LoadKeysResponse, MultiCodeSaleRequest, RefundResponse,
    SaleRequest, SelfServiceMultiCodeSaleResponse, SelfServiceSaleResponse,
    initialization_result_payload, initialize_payload, load_keys_payload, parse_close,
    parse_initialization, parse_load_keys, parse_refund, parse_self_service_multicode_sale,
    parse_self_service_sale, poll_payload, self_service_close_payload,
    self_service_last_sale_payload, self_service_multicode_sale_payload,
    self_service_refund_payload, self_service_sale_payload,
};
use crate::protocol::{
    FC_CLOSE_RESPONSE, FC_INITIALIZATION_RESPONSE, FC_LAST_SALE_RESPONSE, FC_LOAD_KEYS_RESPONSE,
    FC_MULTICODE_SALE_RESPONSE, FC_REFUND_RESPONSE, FC_SALE_RESPONSE,
};
use crate::transport::{Clock, SystemClock, Transport};

fn ignore_status(_: IntermediateStatus) {}

/// Client of a POS Autoservicio.
///
/// Every operation validates its arguments, sends the command through the
/// [`Link`] and parses the typed response. A declined card is an `Ok` response whose
/// `is_approved()` is `false`; `Err` means a communication or protocol failure
/// (ADR-03). One operation at a time: every method takes `&mut self`.
///
/// ```
/// use transbank_pos::{LinkConfig, PosAutoservicio, SaleRequest};
/// use transbank_pos::transport::ManualClock;
/// use transbank_pos_sim::{PosSimulator, SimTransport, TerminalKind};
///
/// let clock = ManualClock::new();
/// let transport = SimTransport::new(PosSimulator::new(TerminalKind::Autoservicio), clock.clone());
/// let mut pos = PosAutoservicio::with_clock(transport, LinkConfig::default(), clock);
///
/// pos.poll()?;
/// let sale = pos.sale(&SaleRequest::new(12_000, "ABC123")?)?;
/// if sale.is_approved() {
///     assert_eq!(sale.amount, Some(12_000));
/// }
/// # Ok::<(), transbank_pos::Error>(())
/// ```
pub struct PosAutoservicio<T: Transport, C: Clock = SystemClock> {
    link: Link<T, C>,
}

impl<T: Transport> PosAutoservicio<T> {
    /// Creates a client with the default [`LinkConfig`] (2 s ACK, 150 s response).
    #[must_use]
    pub fn new(transport: T) -> Self {
        Self::with_config(transport, LinkConfig::default())
    }

    /// Creates a client with a custom [`LinkConfig`].
    #[must_use]
    pub fn with_config(transport: T, config: LinkConfig) -> Self {
        Self::with_clock(transport, config, SystemClock)
    }
}

impl<T: Transport, C: Clock> PosAutoservicio<T, C> {
    /// Creates a client with an explicit clock (tests use a
    /// [`ManualClock`](crate::transport::ManualClock)).
    #[must_use]
    pub fn with_clock(transport: T, config: LinkConfig, clock: C) -> Self {
        let mut link = Link::with_clock(transport, config, clock);
        link.set_trace_layout(TraceLayout::Autoservicio);
        Self { link }
    }

    /// Checks that the POS is connected and responsive (`0100`, ACK only).
    ///
    /// # Errors
    ///
    /// [`Error::NoAck`](crate::Error::NoAck) if the POS does not acknowledge, or a
    /// transport error.
    pub fn poll(&mut self) -> Result<()> {
        self.link.send_command(&poll_payload())
    }

    /// Loads the encryption keys from Transbank (`0800`).
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

    /// Initializes the POS Autoservicio (`0070`, ACK only).
    ///
    /// Call [`initialization_result`](Self::initialization_result) to query the result.
    ///
    /// # Errors
    ///
    /// [`Error::NoAck`](crate::Error::NoAck) if the POS does not acknowledge, or a
    /// transport error.
    pub fn initialize(&mut self) -> Result<()> {
        self.link.send_command(&initialize_payload())
    }

    /// Queries the initialization result of the POS Autoservicio (`0080` -> `1080`).
    ///
    /// # Errors
    ///
    /// Communication and protocol errors of [`Link::request`].
    pub fn initialization_result(&mut self) -> Result<InitializationResponse> {
        let response = self.link.request(
            &initialization_result_payload(),
            FC_INITIALIZATION_RESPONSE,
            &mut ignore_status,
        )?;
        parse_initialization(&response)
    }

    /// Sale (`0200`), without intermediate messages.
    ///
    /// # Timeouts y doble cobro
    ///
    /// If the call fails with [`Error::ResponseTimeout`](crate::Error::ResponseTimeout)
    /// or an I/O error after the POS acknowledged the sale, **the result is unknown**:
    /// the customer may have paid. The client never resends a sale by itself (ADR-04, LINK-03).
    /// Query [`last_sale`](Self::last_sale) and compare ticket and amount before
    /// retrying:
    ///
    /// ```
    /// use transbank_pos::{Error, LinkConfig, PosAutoservicio, SaleRequest};
    /// use transbank_pos::transport::ManualClock;
    /// use transbank_pos_sim::{PosSimulator, Scenario, SimTransport, TerminalKind};
    ///
    /// let clock = ManualClock::new();
    /// let sim = PosSimulator::new(TerminalKind::Autoservicio).with_scenario(Scenario::NoResponse);
    /// let mut pos = PosAutoservicio::with_clock(SimTransport::new(sim, clock.clone()), LinkConfig::default(), clock);
    /// let request = SaleRequest::new(12_000, "ABC123")?;
    ///
    /// match pos.sale(&request) {
    ///     Err(Error::ResponseTimeout { .. }) => {
    ///         pos.link_mut().transport_mut().sim_mut().set_scenario(Scenario::Approve);
    ///         let last = pos.last_sale(false)?;
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
    pub fn sale(&mut self, req: &SaleRequest) -> Result<SelfServiceSaleResponse> {
        let response = self.link.request(
            &self_service_sale_payload(req, false),
            FC_SALE_RESPONSE,
            &mut ignore_status,
        )?;
        parse_self_service_sale(&response)
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
    ) -> Result<SelfServiceSaleResponse> {
        let response = self.link.request(
            &self_service_sale_payload(req, true),
            FC_SALE_RESPONSE,
            &mut on_status,
        )?;
        parse_self_service_sale(&response)
    }

    /// Multi-code sale (`0270`) for a child commerce. See [`sale`](Self::sale) about
    /// timeouts.
    ///
    /// # Errors
    ///
    /// Same as [`sale`](Self::sale).
    pub fn multicode_sale(
        &mut self,
        req: &MultiCodeSaleRequest,
    ) -> Result<SelfServiceMultiCodeSaleResponse> {
        let response = self.link.request(
            &self_service_multicode_sale_payload(req, false),
            FC_MULTICODE_SALE_RESPONSE,
            &mut ignore_status,
        )?;
        parse_self_service_multicode_sale(&response)
    }

    /// Last sale registered by the POS Autoservicio (`0250`), optionally requesting
    /// the voucher.
    ///
    /// # Errors
    ///
    /// Communication and protocol errors of [`Link::request`].
    pub fn last_sale(&mut self, voucher: bool) -> Result<SelfServiceSaleResponse> {
        let response = self.link.request(
            &self_service_last_sale_payload(voucher),
            FC_LAST_SALE_RESPONSE,
            &mut ignore_status,
        )?;
        parse_self_service_sale(&response)
    }

    /// Refunds the last credit sale (`1200`). Never retried automatically (ADR-04).
    ///
    /// # Errors
    ///
    /// Communication and protocol errors of [`Link::request`].
    pub fn refund_last(&mut self) -> Result<RefundResponse> {
        let response = self.link.request(
            &self_service_refund_payload(),
            FC_REFUND_RESPONSE,
            &mut ignore_status,
        )?;
        parse_refund(&response)
    }

    /// Day close (`0500`), optionally requesting the voucher.
    ///
    /// # Errors
    ///
    /// Communication and protocol errors of [`Link::request`].
    pub fn close(&mut self, voucher: bool) -> Result<CloseResponse> {
        let response = self.link.request(
            &self_service_close_payload(voucher),
            FC_CLOSE_RESPONSE,
            &mut ignore_status,
        )?;
        parse_close(&response)
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
