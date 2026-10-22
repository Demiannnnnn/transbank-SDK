//! Response codes (`ResponseCode`) and intermediate status messages (`IntermediateStatus`).

use std::fmt;

macro_rules! response_codes {
    ($($(#[$meta:meta])* $variant:ident = $code:literal => $message:literal,)+) => {
        /// Response code reported by the POS (`docs/protocol-spec.md` §9).
        ///
        /// A non-approved code is a business result, not an [`Error`](crate::Error)
        /// (ADR-03). Unknown codes are kept in [`ResponseCode::Other`].
        ///
        /// ```
        /// use transbank_pos::ResponseCode;
        ///
        /// let code = ResponseCode::from_code(7);
        /// assert_eq!(code, ResponseCode::CanceledAtPos);
        /// assert!(!code.is_approved());
        /// assert_eq!(code.to_string(), "07 Transacción Cancelada desde el POS");
        /// assert_eq!(ResponseCode::from_code(99), ResponseCode::Other(99));
        /// ```
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize))]
        #[non_exhaustive]
        pub enum ResponseCode {
            $(
                #[doc = concat!("`", stringify!($code), "` — ", $message, ".")]
                $(#[$meta])*
                $variant,
            )+
            /// Code not listed in the specification, as received.
            Other(u16),
        }

        impl ResponseCode {
            /// Every code of the specification table, in order.
            #[cfg(test)]
            pub(crate) const KNOWN: &'static [ResponseCode] = &[$(Self::$variant,)+];

            /// Maps a numeric code; unknown codes become [`ResponseCode::Other`].
            #[must_use]
            pub fn from_code(code: u16) -> Self {
                match code {
                    $($code => Self::$variant,)+
                    other => Self::Other(other),
                }
            }

            /// Numeric code.
            #[must_use]
            pub fn code(self) -> u16 {
                match self {
                    $(Self::$variant => $code,)+
                    Self::Other(code) => code,
                }
            }

            /// Message in Spanish as reported by Transbank; `"Código desconocido"` for
            /// [`ResponseCode::Other`].
            #[must_use]
            pub fn message(self) -> &'static str {
                match self {
                    $(Self::$variant => $message,)+
                    Self::Other(_) => "Código desconocido",
                }
            }
        }
    };
}

// Messages from `responseCodes.js` of the official Node SDK (transbank-pos-sdk-nodejs,
// BSD-3-Clause; see NOTICE), as listed in docs/protocol-spec.md §9.
response_codes! {
    Approved = 0 => "Aprobado",
    Rejected = 1 => "Rechazado",
    HostNotResponding = 2 => "Host no Responde",
    ConnectionFailed = 3 => "Conexión Fallo",
    AlreadyRefunded = 4 => "Transacción ya Fue Anulada",
    NoTransactionToRefund = 5 => "No existe Transacción para Anular",
    CardNotSupported = 6 => "Tarjeta no Soportada",
    CanceledAtPos = 7 => "Transacción Cancelada desde el POS",
    DebitCannotBeRefunded = 8 => "No puede Anular Transacción Debito",
    CardReadError = 9 => "Error Lectura Tarjeta",
    AmountBelowMinimum = 10 => "Monto menor al mínimo permitido",
    NoSale = 11 => "No existe venta",
    TransactionNotSupported = 12 => "Transacción No Soportada",
    MustClose = 13 => "Debe ejecutar cierre",
    NoDialTone = 14 => "No hay Tono",
    BitmapFileNotFound = 15 => "Archivo BITMAP.DAT no encontrado. Favor cargue",
    HostResponseFormatError = 16 => "Error Formato Respuesta del HOST",
    LastFourDigitsError = 17 => "Error en los 4 últimos dígitos.",
    InvalidMenu = 18 => "Menú invalido",
    CardDistError = 19 => "ERROR_TARJ_DIST",
    InvalidCard = 20 => "Tarjeta Invalida",
    RefundNotAllowed = 21 => "Anulación no Permitida",
    Timeout = 22 => "TIMEOUT",
    PrinterOutOfPaper = 24 => "Impresora Sin Papel",
    InvalidDate = 25 => "Fecha Invalida",
    MustLoadKeys = 26 => "Debe Cargar Llaves",
    MustUpdate = 27 => "Debe Actualizar",
    InstallmentsNumberError = 60 => "Error en Número de Cuotas",
    RequestBuildError = 61 => "Error en Armado de Solicitud",
    InternalPinpadError = 62 => "Problema con el Pinpad interno",
    HostResponseProcessingError = 65 => "Error al Procesar la Respuesta del Host",
    MaxSalesExceeded = 67 => "Superó Número Máximo de Ventas, Debe Ejecutar Cierre",
    AmountEntryError = 68 => "Error Genérico, Falla al Ingresar Montos",
    TicketFormatError = 70 => "Error de formato Campo de Boleta MAX 6",
    PrintFieldLengthError = 71 => "Error de Largo Campo de Impresión",
    SaleAmountMustBePositive = 72 => "Error de Monto Venta, Debe ser Mayor que 0",
    TerminalIdNotConfigured = 73 => "Terminal ID no configurado",
    MustRunClose = 74 => "Debe Ejecutar CIERRE",
    NoCardsConfigured = 75 => "Comercio no tiene Tarjetas Configuradas",
    MaxSalesReached = 76 => "Supero Número Máximo de Ventas, Debe Ejecutar CIERRE",
    MustExecuteClose = 77 => "Debe Ejecutar Cierre",
    WaitingForCard = 78 => "Esperando Leer Tarjeta",
    ConfirmingAmount = 79 => "Solicitando Confirmar Monto",
    InstallmentsSelection = 80 => "Selección de Cuotas",
    RequestingPin = 81 => "Solicitando Ingreso de Clave",
    SendingToHost = 82 => "Enviando transacción al Host",
    CreditOrDebitSelection = 83 => "Seleccion menu credito/redcompra",
    OperateCard = 84 => "Opere tarjeta",
    SelectingInstallments = 85 => "Seleccion de cuotas",
    EnteringInstallments = 86 => "Ingreso de cuotas",
    ConfirmingInstallments = 87 => "Confirmacion de cuotas",
    InstallmentsCountError = 88 => "Error Cantidad Cuotas",
    InitializationOk = 90 => "Inicialización OK",
    Declined = 93 => "Declinada",
    ResponseProcessingError = 94 => "Error al Procesar Respuesta",
    TasaPrintError = 95 => "Error al Imprimir TASA",
}

impl ResponseCode {
    /// `true` only for [`Approved`](Self::Approved) and, in the self-service
    /// initialization, [`InitializationOk`](Self::InitializationOk).
    #[must_use]
    pub fn is_approved(self) -> bool {
        matches!(self, Self::Approved | Self::InitializationOk)
    }

    /// `true` when the POS asks for a day close before more sales (13, 67, 74, 76, 77).
    #[must_use]
    pub fn requires_close(self) -> bool {
        matches!(
            self,
            Self::MustClose
                | Self::MaxSalesExceeded
                | Self::MustRunClose
                | Self::MaxSalesReached
                | Self::MustExecuteClose
        )
    }

    /// `true` when the POS asks to load keys (26).
    #[must_use]
    pub fn requires_load_keys(self) -> bool {
        self == Self::MustLoadKeys
    }
}

impl fmt::Display for ResponseCode {
    /// `"{code:02} {message}"`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02} {}", self.code(), self.message())
    }
}

/// Intermediate status message (`0900`) sent by the POS during a sale.
///
/// ```
/// use transbank_pos::{IntermediateStatus, ResponseCode};
///
/// let status = IntermediateStatus::Code(ResponseCode::RequestingPin);
/// assert_eq!(status.message(), "Solicitando Ingreso de Clave");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub enum IntermediateStatus {
    /// A code of the specification table.
    Code(ResponseCode),
    /// An unknown or non-numeric code, as received.
    Unknown(String),
}

impl IntermediateStatus {
    /// Human-readable message: the code message, or the raw text if unknown.
    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::Code(code) => code.message(),
            Self::Unknown(text) => text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_known_code_round_trips() {
        assert_eq!(ResponseCode::KNOWN.len(), 56);
        for &code in ResponseCode::KNOWN {
            assert_eq!(ResponseCode::from_code(code.code()), code);
            assert_ne!(code.message(), "Código desconocido");
        }
        assert_eq!(ResponseCode::from_code(23), ResponseCode::Other(23));
        assert_eq!(ResponseCode::Other(23).code(), 23);
        assert_eq!(ResponseCode::Other(23).message(), "Código desconocido");
    }

    #[test]
    fn classification() {
        assert!(ResponseCode::Approved.is_approved());
        assert!(ResponseCode::InitializationOk.is_approved());
        assert!(!ResponseCode::Rejected.is_approved());
        let close: Vec<u16> = ResponseCode::KNOWN
            .iter()
            .filter(|code| code.requires_close())
            .map(|code| code.code())
            .collect();
        assert_eq!(close, [13, 67, 74, 76, 77]);
        assert!(ResponseCode::from_code(26).requires_load_keys());
    }

    #[test]
    fn display() {
        assert_eq!(ResponseCode::Approved.to_string(), "00 Aprobado");
        assert_eq!(
            ResponseCode::from_code(81).to_string(),
            "81 Solicitando Ingreso de Clave"
        );
    }
}
