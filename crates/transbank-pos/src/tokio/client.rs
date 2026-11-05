//! [`AsyncPosIntegrado`]: asynchronous typed client of the POS Integrado (`docs/protocol-spec.md` §4).

use std::time::Duration;

use ::tokio::io::{AsyncRead, AsyncWrite};

use super::link::AsyncLink;
use crate::LinkConfig;
use crate::codes::IntermediateStatus;
use crate::error::{Error, Result};
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

fn ignore_status(_: IntermediateStatus) {}

/// Asynchronous client of a POS Integrado over an [`AsyncRead`] + [`AsyncWrite`] stream.
///
/// Every operation validates its arguments, sends the command through the
/// [`AsyncLink`] and parses the typed response. A declined card is an `Ok` response
/// whose `is_approved()` is `false`; `Err` means a communication or protocol failure
/// (ADR-03).
///
/// # Cancellation safety
///
/// Dropping the future of an ongoing transaction (such as a [`sale`](Self::sale))
/// leaves the transaction in an **unknown** state on the POS: the customer may
/// have already completed payment. In such cases, query [`last_sale`](Self::last_sale)
/// before attempting any retry.
///
/// ```no_run
/// # #[cfg(feature = "tokio")]
/// # async fn doc() -> Result<(), transbank_pos::Error> {
/// use transbank_pos::SaleRequest;
/// use transbank_pos::tokio::AsyncPosIntegrado;
///
/// let mut pos = AsyncPosIntegrado::connect_tcp("127.0.0.1:7777").await?;
/// pos.poll().await?;
/// let sale = pos.sale(&SaleRequest::new(12_000, "ABC123")?).await?;
/// if sale.is_approved() {
///     // payment approved
/// }
/// # Ok(())
/// # }
/// ```
pub struct AsyncPosIntegrado<S: AsyncRead + AsyncWrite + Unpin + Send> {
    link: AsyncLink<S>,
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send> AsyncPosIntegrado<S> {
    /// Creates a client with the default [`LinkConfig`] (2 s ACK, 150 s response).
    #[must_use]
    pub fn new(stream: S) -> Self {
        Self::with_config(stream, LinkConfig::default())
    }

    /// Creates a client with a custom [`LinkConfig`].
    #[must_use]
    pub fn with_config(stream: S, config: LinkConfig) -> Self {
        Self {
            link: AsyncLink::new(stream, config),
        }
    }

    /// Checks that the POS is connected and in integrated mode (`0100`, ACK only).
    ///
    /// # Errors
    ///
    /// [`Error::NoAck`] if the POS does not acknowledge, or an I/O error.
    pub async fn poll(&mut self) -> Result<()> {
        self.link.send_command(&poll_payload()).await
    }

    /// Loads the keys from Transbank (`0800`); recommended at the start of the day
    /// and when a response asks for it (code 26).
    ///
    /// # Errors
    ///
    /// Communication and protocol errors of [`AsyncLink::request`].
    pub async fn load_keys(&mut self) -> Result<LoadKeysResponse> {
        let mut ignore = ignore_status;
        let response = self
            .link
            .request(&load_keys_payload(), FC_LOAD_KEYS_RESPONSE, &mut ignore)
            .await?;
        parse_load_keys(&response)
    }

    /// Sale (`0200`), without intermediate messages.
    ///
    /// # Timeouts and cancellation (avoiding double charges)
    ///
    /// If the call fails with [`Error::ResponseTimeout`] or an I/O error, or if the
    /// future is dropped/canceled while waiting, **the result is unknown**: the customer
    /// may have paid. The client never resends a sale by itself (ADR-04). Query
    /// [`last_sale`](Self::last_sale) and compare ticket and amount before retrying.
    ///
    /// # Errors
    ///
    /// Communication and protocol errors of [`AsyncLink::request`]. A declined card is
    /// `Ok` with `is_approved() == false`.
    pub async fn sale(&mut self, req: &SaleRequest) -> Result<SaleResponse> {
        let mut ignore = ignore_status;
        let response = self
            .link
            .request(&sale_payload(req, false), FC_SALE_RESPONSE, &mut ignore)
            .await?;
        parse_sale(&response)
    }

    /// Sale (`0200`) asking the POS for intermediate messages (`0900`), delivered to
    /// `on_status` while the customer operates the POS. See [`sale`](Self::sale)
    /// about timeouts and cancellation.
    ///
    /// # Errors
    ///
    /// Same as [`sale`](Self::sale).
    pub async fn sale_with_status(
        &mut self,
        req: &SaleRequest,
        mut on_status: impl FnMut(IntermediateStatus) + Send,
    ) -> Result<SaleResponse> {
        let response = self
            .link
            .request(&sale_payload(req, true), FC_SALE_RESPONSE, &mut on_status)
            .await?;
        parse_sale(&response)
    }

    /// Multi-code sale (`0270`) for a child commerce. See [`sale`](Self::sale) about
    /// timeouts and cancellation (use [`multicode_last_sale`](Self::multicode_last_sale)).
    ///
    /// # Errors
    ///
    /// Same as [`sale`](Self::sale).
    pub async fn multicode_sale(
        &mut self,
        req: &MultiCodeSaleRequest,
    ) -> Result<MultiCodeSaleResponse> {
        let mut ignore = ignore_status;
        self.request_multicode_sale(req, false, &mut ignore).await
    }

    /// Multi-code sale (`0270`) with intermediate messages.
    ///
    /// # Errors
    ///
    /// Same as [`sale`](Self::sale).
    pub async fn multicode_sale_with_status(
        &mut self,
        req: &MultiCodeSaleRequest,
        mut on_status: impl FnMut(IntermediateStatus) + Send,
    ) -> Result<MultiCodeSaleResponse> {
        self.request_multicode_sale(req, true, &mut on_status).await
    }

    async fn request_multicode_sale(
        &mut self,
        req: &MultiCodeSaleRequest,
        status: bool,
        on_status: &mut (dyn FnMut(IntermediateStatus) + Send),
    ) -> Result<MultiCodeSaleResponse> {
        let response = self
            .link
            .request(
                &multicode_sale_payload(req, status),
                FC_MULTICODE_SALE_RESPONSE,
                on_status,
            )
            .await?;
        parse_multicode_sale(&response)
    }

    /// Last sale registered by the POS (`0250`); the way to resolve a sale whose
    /// result is unknown. Code 11 means there is no sale.
    ///
    /// # Errors
    ///
    /// Communication and protocol errors of [`AsyncLink::request`].
    pub async fn last_sale(&mut self) -> Result<SaleResponse> {
        let mut ignore = ignore_status;
        let response = self
            .link
            .request(&last_sale_payload(), FC_LAST_SALE_RESPONSE, &mut ignore)
            .await?;
        parse_sale(&response)
    }

    /// Last multi-code sale (`0280`), optionally with its voucher.
    ///
    /// # Errors
    ///
    /// Communication and protocol errors of [`AsyncLink::request`].
    pub async fn multicode_last_sale(&mut self, voucher: bool) -> Result<MultiCodeSaleResponse> {
        let mut ignore = ignore_status;
        let response = self
            .link
            .request(
                &multicode_last_sale_payload(voucher),
                FC_MULTICODE_LAST_SALE_RESPONSE,
                &mut ignore,
            )
            .await?;
        parse_multicode_sale(&response)
    }

    /// Refunds a credit sale by its operation number (`1200`). Debit sales cannot be
    /// refunded (code 8). Never retried automatically (ADR-04).
    ///
    /// # Errors
    ///
    /// Communication and protocol errors of [`AsyncLink::request`].
    pub async fn refund(&mut self, operation: OperationNumber) -> Result<RefundResponse> {
        let mut ignore = ignore_status;
        let response = self
            .link
            .request(&refund_payload(operation), FC_REFUND_RESPONSE, &mut ignore)
            .await?;
        parse_refund(&response)
    }

    /// Number and amount of the sales since the last close (`0700`).
    ///
    /// # Errors
    ///
    /// Communication and protocol errors of [`AsyncLink::request`].
    pub async fn totals(&mut self) -> Result<TotalsResponse> {
        let mut ignore = ignore_status;
        let response = self
            .link
            .request(&totals_payload(), FC_TOTALS_RESPONSE, &mut ignore)
            .await?;
        parse_totals(&response)
    }

    /// Sales detail (`0260`). [`SalesDetailMode::PrintOnPos`] prints it on the POS
    /// and returns an empty list; [`SalesDetailMode::SendToHost`] returns one entry
    /// per sale.
    ///
    /// # Errors
    ///
    /// Communication and protocol errors of [`AsyncLink::request_list`].
    pub async fn sales_detail(&mut self, mode: SalesDetailMode) -> Result<Vec<DetailEntry>> {
        let payload = sales_detail_payload(mode);
        match mode {
            SalesDetailMode::PrintOnPos => {
                self.link.send_command(&payload).await.map(|()| Vec::new())
            }
            SalesDetailMode::SendToHost => {
                let responses = self
                    .link
                    .request_list(&payload, FC_SALES_DETAIL_RESPONSE)
                    .await?;
                responses
                    .iter()
                    .map(|entry| parse_detail_entry(entry))
                    .collect()
            }
        }
    }

    /// Multi-code sales detail (`0290`); see [`sales_detail`](Self::sales_detail).
    ///
    /// # Errors
    ///
    /// Communication and protocol errors of [`AsyncLink::request_list`].
    pub async fn multicode_sales_detail(
        &mut self,
        mode: SalesDetailMode,
    ) -> Result<Vec<MultiCodeDetailEntry>> {
        let payload = multicode_sales_detail_payload(mode);
        match mode {
            SalesDetailMode::PrintOnPos => {
                self.link.send_command(&payload).await.map(|()| Vec::new())
            }
            SalesDetailMode::SendToHost => {
                let responses = self
                    .link
                    .request_list(&payload, FC_MULTICODE_SALES_DETAIL_RESPONSE)
                    .await?;
                responses
                    .iter()
                    .map(|entry| parse_multicode_detail_entry(entry))
                    .collect()
            }
        }
    }

    /// Day close (`0500`): the POS prints the report and starts a new period.
    ///
    /// # Errors
    ///
    /// Communication and protocol errors of [`AsyncLink::request`].
    pub async fn close(&mut self) -> Result<CloseResponse> {
        let mut ignore = ignore_status;
        let response = self
            .link
            .request(&close_payload(), FC_CLOSE_RESPONSE, &mut ignore)
            .await?;
        parse_close(&response)
    }

    /// Takes the POS out of integrated mode (`0300`, ACK only). It has to be
    /// re-enabled from the POS menu.
    ///
    /// # Errors
    ///
    /// [`Error::NoAck`] or an I/O error.
    pub async fn set_normal_mode(&mut self) -> Result<()> {
        self.link.send_command(&normal_mode_payload()).await
    }

    /// The underlying asynchronous link.
    pub fn link_mut(&mut self) -> &mut AsyncLink<S> {
        &mut self.link
    }

    /// Consumes the client and returns the underlying stream.
    #[must_use]
    pub fn into_inner(self) -> S {
        self.link.into_inner()
    }
}

impl AsyncPosIntegrado<::tokio::net::TcpStream> {
    /// Connects to a POS or serial-to-network bridge over TCP with `TCP_NODELAY` and
    /// a 5-second connection timeout.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if the address cannot be resolved, the connection times out,
    /// or setting `TCP_NODELAY` fails.
    pub async fn connect_tcp(addr: impl ::tokio::net::ToSocketAddrs) -> Result<Self> {
        let connect_timeout = Duration::from_secs(5);
        let stream =
            match ::tokio::time::timeout(connect_timeout, ::tokio::net::TcpStream::connect(addr))
                .await
            {
                Ok(Ok(stream)) => stream,
                Ok(Err(err)) => return Err(Error::Io(err)),
                Err(_) => {
                    return Err(Error::Io(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "TCP connection timed out after 5 s",
                    )));
                }
            };
        stream.set_nodelay(true)?;
        Ok(Self::new(stream))
    }
}

#[cfg(feature = "tokio-serial")]
impl AsyncPosIntegrado<::tokio_serial::SerialStream> {
    /// Opens a serial port with 8N1 framing and no flow control.
    ///
    /// # Errors
    ///
    /// [`Error::Transport`] if opening or configuring the serial port fails.
    pub fn open_serial(path: &str, baud_rate: u32) -> Result<Self> {
        let builder = ::tokio_serial::new(path, baud_rate)
            .data_bits(::tokio_serial::DataBits::Eight)
            .flow_control(::tokio_serial::FlowControl::None)
            .parity(::tokio_serial::Parity::None)
            .stop_bits(::tokio_serial::StopBits::One);
        let stream = ::tokio_serial::SerialStream::open(&builder)
            .map_err(|err| Error::Transport(Box::new(err)))?;
        Ok(Self::new(stream))
    }
}
