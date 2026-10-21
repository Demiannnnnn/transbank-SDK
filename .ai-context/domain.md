# Dominio — transbank-SDK

## 1. Qué es

Una librería Rust para que un software de punto de venta (caja, POS de un comercio, kiosco de autoatención) **cobre con tarjeta** usando un terminal físico de Transbank conectado al computador. El software dice "cobra $12.000 de la boleta ABC123"; el terminal pide la tarjeta al cliente, habla con Transbank y devuelve el resultado. El SDK es el traductor entre ambos.

Motivación: el dueño necesita esta integración en `rework-sistema-facturacion` (app de escritorio Tauri/Rust) y Transbank no publica SDK para Rust. Decisión del dueño (2026-10-03): hacerlo como **librería pública, genérica, sin tipos de ese proyecto**, con licencia MIT/Apache-2.0, para portafolio y para la comunidad. No es un negocio.

## 2. Actores

| Actor | Rol |
|:--|:--|
| **Comercio** | Tiene contrato con Transbank, un código de comercio (`commerce_code`, 12 dígitos) y uno o más terminales. |
| **Host** | El computador con el software de caja que usa este SDK. |
| **POS** | Terminal de Transbank. Lee la tarjeta, pide la clave, imprime vouchers y habla con Transbank por su propia red. **El host nunca ve datos completos de la tarjeta.** |
| **Cliente** | Persona que paga con su tarjeta en el POS. |
| **Transbank** | Adquirente: autoriza o rechaza. El host nunca habla con Transbank directamente. |
| **Desarrollador** | Quien integra el SDK en su software. Usuario principal de la API. |

## 3. Tipos de terminal

| Tipo | Uso | Baudios | Particularidades |
|:--|:--|:--|:--|
| **POS Integrado** | Caja atendida: el cajero inicia el cobro desde el software | 115200 | Imprime el voucher en el propio POS; anulación por número de operación; totales y detalle del día |
| **POS Autoservicio** | Kioscos y máquinas sin cajero | 19200 | Puede devolver el voucher al host para imprimirlo en la máquina; requiere inicialización; anula solo la última venta |

Ambos usan el **mismo enlace** (tramas, ACK/NAK). Cambian los comandos y el orden de los campos (`docs/protocol-spec.md` §4–§7).

## 4. Operaciones (POS Integrado)

| Operación | Para qué | Cuándo |
|:--|:--|:--|
| **Poll** | Ver si el POS está conectado y en modo integrado | Al conectar; antes de vender |
| **Cargar llaves** | Sincronizar llaves de cifrado con Transbank | Al iniciar el día o si el POS responde código 26 |
| **Venta** | Cobrar un monto | Cada pago con tarjeta |
| **Venta multicódigo** | Cobrar a nombre de un comercio hijo (centros comerciales, marketplaces) | Comercios con multicódigo |
| **Última venta** | Recuperar el resultado de la última venta | Tras un timeout o un corte: **antes de reintentar** |
| **Anulación** | Revertir una venta de **crédito** por su número de operación | Devoluciones el mismo día (débito no se anula) |
| **Totales** | Cantidad y monto vendidos desde el último cierre | Arqueo de caja |
| **Detalle de ventas** | Lista de ventas del día (al host o impresa) | Arqueo, conciliación |
| **Cierre** | Cerrar el día en Transbank; el POS imprime el reporte | Fin del día; si el POS lo exige (códigos 13, 67, 74, 76, 77) |
| **Modo normal** | Sacar al POS del modo integrado | Rara vez (para usarlo sin software) |

## 5. Datos de una venta

| Dato | Significado |
|:--|:--|
| Monto | Pesos chilenos enteros (CLP no tiene decimales). Mínimo 50, máximo 999.999.999 |
| Ticket | Identificador de la venta en el software del comercio (número de boleta), hasta 6 caracteres alfanuméricos. Aparece en el voucher |
| Código de respuesta | Resultado: `0` aprobado; otros, rechazo o error (`protocol-spec.md` §9) |
| Código de autorización | Lo da Transbank al aprobar. Vacío si no se aprobó |
| Número de operación | Correlativo del POS; se usa para anular |
| Últimos 4 dígitos | De la tarjeta; lo único de la tarjeta que ve el host |
| Tipo de tarjeta | Crédito (`CR`), débito (`DB`), prepago (`PR`) |
| Marca | Visa (`VI`), Mastercard (`MC`), Amex (`AX`), Redcompra/débito (`DB`)… |
| Cuotas | Número y monto de cuotas, si el cliente eligió cuotas |
| Fecha contable / fecha real | Día contable de Transbank y momento real de la transacción (hora local del POS, sin zona) |
| Voucher | Texto del comprobante, líneas de 40 caracteres, si se pidió |

## 6. Reglas de negocio que el SDK protege

1. **Nunca doble cobro.** Si una venta no tiene respuesta clara (timeout, corte), el SDK no la reintenta: el software debe consultar la última venta y comparar ticket y monto (`protocol-spec.md` §8).
2. **Rechazo ≠ error.** Una tarjeta rechazada es un resultado normal (`Ok` con código ≠ 0). `Err` es solo para fallas de comunicación o de protocolo.
3. **Una operación a la vez** por conexión: el POS no acepta comandos concurrentes.
4. **Montos enteros en CLP**, sin `f32`/`f64`.

## 7. Alcance — qué NO es

- **No** es Webpay, Webpay Plus, Oneclick ni ninguna API web de Transbank (para eso existen SDKs oficiales HTTP).
- **No** procesa ni almacena datos de tarjetas; no está en alcance PCI: el POS hace todo eso.
- **No** genera boletas ni documentos tributarios, ni habla con el SII.
- **No** es oficial ni lo soporta Transbank. No reemplaza la certificación que Transbank pueda exigir al comercio.
- **No** incluye (v0.1) Bluetooth, POS Android "Smart" ni integraciones por nube (Mercado Pago Point, Getnet): pueden ser crates futuros.
- **No** incluye UI.
