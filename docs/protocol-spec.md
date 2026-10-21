# Especificación del protocolo — Transbank POS Integrado y Autoservicio

**Documento normativo.** Describe los bytes que viajan entre el computador (el *host*, esta librería) y el terminal POS de Transbank conectado por USB o RS-232. Si el código y este documento difieren, es un bug en uno de los dos: repórtalo.

Fuente: ingeniería inversa **de los SDKs oficiales** de Transbank (licencia BSD-3-Clause), revisados el 2026-10-21:

| SDK | Repositorio | Qué se tomó |
|:--|:--|:--|
| Node.js | `TransbankDevelopers/transbank-pos-sdk-nodejs` (`src/PosBase.js`, `PosIntegrado.js`, `PosAutoservicio.js`, `responseCodes.js`) | Comandos, timeouts (ACK 2 s, respuesta 150 s), baudios por defecto, códigos de respuesta |
| .NET | `TransbankDevelopers/transbank-pos-sdk-dotnet` (`TransbankPosSDK/Services/PosService.cs`, `Responses/**`, `Utils/VoucherParser.cs`, tests) | Índices de campos de cada respuesta, NAK ante LRC inválido, fin del listado de detalle, voucher de 40 columnas, ejemplos de tramas |
| C | `TransbankDevelopers/transbank-pos-sdk-c` (`src/hex_codes.h`, `transbank_serial_utils.c`, `transbank.c`) | Bytes de control, cálculo del LRC, 3 reintentos, baudios soportados |

No existe en este repo documentación oficial del protocolo. Los puntos donde los SDKs no coinciden están en §10 (**preguntas abiertas**) y se cierran solo con un POS real (`docs/hardware-testing.md`).

---

## 1. Capa física

| Parámetro | Valor |
|:--|:--|
| Conexión | USB (el POS aparece como puerto serie CDC-ACM: `COMx` en Windows, `/dev/ttyACM*` o `/dev/ttyUSB*` en Linux, `/dev/cu.usbmodem*` en macOS) o RS-232 |
| Formato | 8 bits de datos, sin paridad, 1 bit de parada (8N1), sin control de flujo |
| Baudios por defecto | **115200** (POS Integrado) · **19200** (POS Autoservicio) |
| Baudios soportados | 1200, 2400, 4800, 9600, 19200, 115200 |
| Modelos citados por los SDKs | Verifone VX520 y VX520c, Ingenico Desk 3500 (Integrado); equipos de Autoservicio |

El POS debe estar en **modo integrado** (lo activa Transbank). En modo normal no responde a comandos.

## 2. Tramas

### 2.1 Bytes de control

| Nombre | Byte | Uso |
|:--|:--|:--|
| `STX` | `0x02` | Inicio de trama |
| `ETX` | `0x03` | Fin de los datos de la trama |
| `ACK` | `0x06` | Acuse de recibo positivo (byte suelto, fuera de trama) |
| `NAK` | `0x15` | Acuse negativo: la trama llegó mal (byte suelto, fuera de trama) |
| `|` | `0x7C` | Separador de campos dentro del payload |

### 2.2 Formato

```
STX  payload (1..N bytes)  ETX  LRC
02   30 31 30 30           03   02      ← trama de "0100"
```

- **Payload:** texto con los campos separados por `|`. El primer campo siempre es el **código de función** de 4 dígitos ASCII.
- **LRC:** XOR de **todos los bytes desde el primero del payload hasta el `ETX` inclusive** (sin el `STX`). Un solo byte.
- El LRC puede valer **cualquier** byte, incluidos `0x02`, `0x03`, `0x06` y `0x15`. Por eso el decodificador lee el byte que sigue al `ETX` como LRC **sin interpretarlo** (§2.4).
- El payload no contiene `STX` ni `ETX`. Al enviar, el host rechaza payloads con bytes `< 0x20` o `0x7F` (error de argumento, nunca se transmiten).
- **Codificación del texto:** el host envía solo ASCII. Al recibir, cada byte se decodifica como ISO-8859-1 (byte = carácter Unicode del mismo valor). Esto nunca falla y conserva las tildes de los vouchers.

### 2.3 Vectores de prueba (normativos)

Los tests de `PROTO-01` DEBEN reproducirlos byte a byte:

| Payload | LRC | Trama completa (hex) |
|:--|:--|:--|
| `0100` | `02` | `02 30 31 30 30 03 02` |
| `0300` | `00` | `02 30 33 30 30 03 00` |
| `0800` | `0B` | `02 30 38 30 30 03 0B` |
| `0500\|\|` | `06` | `02 30 35 30 30 7C 7C 03 06` |
| `0700\|\|` | `04` | `02 30 37 30 30 7C 7C 03 04` |
| `0250\|` | `78` | `02 30 32 35 30 7C 03 78` |
| `0260\|1\|` | `36` | `02 30 32 36 30 7C 31 7C 03 36` |
| `1200\|000142\|` | `07` | `02 31 32 30 30 7C 30 30 30 31 34 32 7C 03 07` |
| `0200\|000012000\|ABC123\|\|0\|0` | `3E` | `02 30 32 30 30 7C 30 30 30 30 31 32 30 30 30 7C 41 42 43 31 32 33 7C 7C 30 7C 30 03 3E` |

Fíjate en `0500||` (LRC = `0x06`, igual que `ACK`) y `0100` (LRC = `0x02`, igual que `STX`): son los casos que rompen a un decodificador ingenuo.

### 2.4 Decodificador (máquina de estados)

El decodificador es *sans-IO*: recibe trozos de bytes de cualquier tamaño y emite eventos. Estados:

| Estado | Byte recibido | Acción |
|:--|:--|:--|
| `Idle` | `ACK` | emite `Ack` |
| `Idle` | `NAK` | emite `Nak` |
| `Idle` | `STX` | pasa a `InFrame`, buffer vacío |
| `Idle` | otro | lo descarta y cuenta `garbage` (emite `Garbage(n)` agrupado al cambiar de estado) |
| `InFrame` | `ETX` | pasa a `AwaitLrc` |
| `InFrame` | `STX` | trama anterior incompleta: emite `Garbage`, reinicia el buffer (sigue en `InFrame`) |
| `InFrame` | otro | lo agrega al buffer; si supera `MAX_PAYLOAD` (**4096** bytes) emite `Overflow` y vuelve a `Idle` |
| `AwaitLrc` | cualquiera | compara con el LRC calculado: igual ⇒ `Frame(payload)`; distinto ⇒ `BadLrc { payload, expected, received }`. Vuelve a `Idle` |

Propiedades obligatorias (tests con `proptest`): partir una secuencia válida en trozos arbitrarios produce los mismos eventos; `decode(encode(p)) == p` para todo payload válido; ningún input hace entrar en pánico al decodificador.

## 3. Capa de enlace (intercambio)

Una sola operación a la vez por conexión. Secuencia de un comando con respuesta:

```
host                               POS
 │── STX 0200|… ETX LRC ─────────▶ │
 │ ◀────────────────────────── ACK │   ≤ ack_timeout (2 s)
 │ ◀──── STX 0900|78 ETX LRC ───── │   mensajes intermedios (solo si se pidieron)
 │── ACK ────────────────────────▶ │
 │ ◀─ STX 0210|00|… ETX LRC ────── │   ≤ response_timeout (150 s)
 │── ACK ────────────────────────▶ │
```

### 3.1 Envío

1. El host escribe la trama.
2. Espera `ACK` hasta `ack_timeout` (por defecto **2000 ms**).
3. `NAK` o vencimiento ⇒ reenvía la **misma** trama. Máximo **3 intentos** en total (`max_attempts`). Agotados ⇒ error `NoAck` (si hubo NAK, el error lo dice).
4. Bytes que no son `ACK`/`NAK` mientras espera el ACK: se descartan y se registran en `trace`.

### 3.2 Recepción de la respuesta

1. Tras el ACK, si el comando tiene respuesta, el host espera tramas hasta `response_timeout` (por defecto **150 s**, el tiempo que el cliente tiene para pasar la tarjeta y digitar la clave).
2. Trama con LRC correcto ⇒ el host responde `ACK` **inmediatamente** (antes de parsear).
3. `BadLrc` ⇒ el host responde `NAK` y sigue esperando la retransmisión. Más de **3** `BadLrc` seguidos ⇒ error `BadLrc`.
4. Código de función `0900` ⇒ mensaje intermedio (§6): se entrega al callback y se sigue esperando (el plazo de `response_timeout` **no** se reinicia).
5. Cualquier otro código ⇒ debe ser el código de respuesta esperado (tabla §4/§7). Si no, error `UnexpectedResponse { expected, received, payload }`.
6. Vencido `response_timeout` ⇒ error `ResponseTimeout`. **Una venta con timeout tiene resultado desconocido:** la librería lo documenta y la CLI sugiere `last-sale` (§8).

### 3.3 Comandos sin respuesta

`0100` (poll), `0300` (cambio a modo normal) y `0070` (inicialización, Autoservicio) terminan al recibir el `ACK`.

## 4. POS Integrado — comandos

Notación: `{amount:09}` = monto en pesos, entero, con ceros a la izquierda hasta 9 dígitos; `{ticket:06}` = número de boleta/ticket (ver §5.1); `{b}` = `1`/`0`.

| Operación | Request (payload) | Respuesta | Notas |
|:--|:--|:--|:--|
| Poll | `0100` | solo ACK | Saber si el POS está conectado y en modo integrado |
| Cargar llaves | `0800` | `0810` | Sincroniza llaves con Transbank; se recomienda al iniciar el día |
| Venta | `0200\|{amount:09}\|{ticket:06}\|\|{voucher}\|{status}` | `0210` | `voucher`: el POS devuelve el voucher en la respuesta. `status`: enviar mensajes intermedios `0900` |
| Venta multicódigo | `0270\|{amount:09}\|{ticket:06}\|\|{voucher}\|{status}\|{commerce_code}` | `0271` | `commerce_code` del comercio hijo; `0` = el principal |
| Última venta | `0250\|` | `0260` | Reconsulta tras un timeout o un corte |
| Última venta multicódigo | `0280\|{voucher}` | `0281` | |
| Anulación | `1200\|{operation:06}\|` | `1210` | `operation` = número de operación de la venta (6 dígitos, ceros a la izquierda). Solo crédito: débito no se anula (código 8) |
| Totales | `0700\|\|` | `0710` | Cantidad y monto de las ventas desde el último cierre |
| Detalle de ventas | `0260\|{report}\|` | `0261` × N | `report` = `0` imprime en el POS (sin respuesta de datos, termina con el ACK); `1` envía cada venta al host (§4.2) |
| Detalle multicódigo | `0290\|{report}\|` | `0291` × N | Igual que el detalle |
| Cierre | `0500\|\|` | `0510` | Cierre del día; el POS imprime el reporte |
| Cambio a modo normal | `0300` | solo ACK | Saca al POS del modo integrado (requiere reactivar desde el POS) |

**Atención — detalle:** en `0260|x|` el SDK oficial invierte el booleano: `printOnPos = true` ⇒ `0`. La API de esta librería expone `SalesDetailMode::{PrintOnPos, SendToHost}` para que nunca se escriba el dígito a mano.

### 4.1 Validaciones del host antes de enviar

| Campo | Regla | Error |
|:--|:--|:--|
| `amount` (venta, multicódigo) | entero `50 ..= 999_999_999` CLP | `InvalidArgument` |
| `ticket` | 1 a 6 caracteres ASCII alfanuméricos `[0-9A-Za-z]`; se rellena con `0` a la izquierda hasta 6 | `InvalidArgument` |
| `operation` (anulación) | entero `1 ..= 999_999` | `InvalidArgument` |
| `commerce_code` (multicódigo) | 1 a 12 dígitos, o `0` | `InvalidArgument` |

### 4.2 Detalle de ventas con `report = 1`

El POS envía una trama `0261` por venta y luego tramas `0261` **sin código de autorización** (campo 5 vacío) que marcan el final. El host:

1. Responde `ACK` a cada trama.
2. Acumula las ventas con autorización no vacía.
3. Al recibir la **primera** trama sin autorización, sigue leyendo y respondiendo `ACK` a tramas sin autorización durante `detail_tail` (por defecto **500 ms** sin bytes) y luego termina. El SDK .NET espera dos tramas vacías; el de Node, una. Esta regla cubre ambos casos (OPEN-03).
4. Si la primera trama tiene código de respuesta `11` ("No existe venta") y autorización vacía ⇒ lista vacía.

## 5. POS Integrado — respuestas

Todas las respuestas comparten: campo `0` = código de función, campo `1` = **código de respuesta** (§9). Reglas de parseo:

- Se divide por `|`. Cada campo se recorta (`trim`) de espacios.
- Campo ausente (la trama es más corta) o vacío ⇒ `None`. Nunca es error, salvo los campos 0 y 1.
- Campo numérico no vacío que no parsea ⇒ error `Parse { field, value }` (no se adivina).
- Fechas `ddMMyyyy` y horas `HHmmss`; se exponen como tipos propios sin zona horaria (`PosDate`, `PosTime`) y opcionalmente combinados (`PosDateTime`). Una fecha inválida como `00-00-00` ⇒ `None` y se conserva el texto crudo.
- Se conserva siempre el payload crudo (`raw`) en la respuesta para diagnóstico.

### 5.1 Venta `0210` (también `0260` última venta)

| # | Campo | Tipo | Ejemplo |
|:--|:--|:--|:--|
| 0 | `function_code` | `0210` | `0210` |
| 1 | `response_code` | código §9 | `00` |
| 2 | `commerce_code` | dígitos | `597029414300` |
| 3 | `terminal_id` | texto | `IT750050` |
| 4 | `ticket` | texto | `ABC123` |
| 5 | `authorization_code` | texto | `757752` |
| 6 | `amount` | entero CLP | `12000` |
| 7 | `installments_number` | entero | `03` |
| 8 | `installments_amount` | entero CLP | `3334` |
| 9 | `last4_digits` | texto de 4 dígitos (se conservan ceros) | `9480` |
| 10 | `operation_number` | entero | `000135` |
| 11 | `card_type` | `CR` crédito · `DB` débito · `PR` prepago · otro ⇒ `Other(String)` | `CR` |
| 12 | `accounting_date` | fecha `ddMMyyyy` | `22102025` |
| 13 | `account_number` | texto (enmascarado por el POS) | `0000000000000000000` |
| 14 | `card_brand` | abreviatura (`VI`, `MC`, `AX`, `DB`, `DC`, `MG`, `TV`, `CA`…) ⇒ `CardBrand` con `Other(String)` | `MC` |
| 15 | `real_date` | fecha `ddMMyyyy` | `22102025` |
| 16 | `real_time` | hora `HHmmss` | `114016` |
| 17 | `employee_id` | entero | `7` |
| 18 | `tip` | entero CLP | `500` |
| 19 | `voucher` | texto: líneas de **40** caracteres concatenadas | (solo si se pidió) |

Ejemplo real (del SDK .NET):
`0210|00|597029414300|IT750050|abc123|757752|12000|03|3334|9480|000135|CR|22102025|0000000000000000000|MC|22102025|114016|7|500|`

Venta rechazada o cancelada: solo los campos 0–3 (o menos) vienen con datos, p. ej. `0271|07|||ABC123||90000|…`. **Una venta rechazada no es un `Err`:** es una respuesta con `response_code != Approved`.

**Voucher:** si el campo 19 tiene largo múltiplo de 40 se parte en líneas de 40; si no, se expone crudo y `voucher_lines()` devuelve `None`.

### 5.2 Venta multicódigo `0271` y última venta multicódigo `0281`

Campos 0–18 iguales a `0210`, más:

| # | Campo | Tipo |
|:--|:--|:--|
| 19 | `filler` / voucher | texto (en `0281` es el voucher) |
| 20 | `change` | entero CLP (vuelto) |
| 21 | `commerce_provider_code` | dígitos (comercio hijo) |

### 5.3 Detalle `0261`

| # | Campo | # | Campo |
|:--|:--|:--|:--|
| 4 | `ticket` | 12 | `card_brand` |
| 5 | `authorization_code` | 13 | `real_date` |
| 6 | `amount` | 14 | `real_time` |
| 7 | `last4_digits` | 15 | `employee_id` |
| 8 | `operation_number` | 16 | `tip` |
| 9 | `card_type` | 17 | `installments_amount` |
| 10 | `accounting_date` | 18 | `installments_number` |
| 11 | `account_number` | | |

Ejemplo: `0261|00|597029414300|IT750050|abc123|757752|12000|9480|000135|CR|22102025|0000000000000000000|MC|22102025|114016|7|500|3334|03`.
Fin de lista: `0261|00|597029414300|IT750050||||||||||||||||` o `0261|11||||||||||||||||||`.

Detalle multicódigo `0291`: campos 4–18 de `0210` (no de `0261`), más `19` = `change`, `20` = `commerce_provider_code`. Ejemplo: `0291|00|597029414300|IT750050|abc123|757752|12000|03|3334|9480|000135|CR|22102025|0000000000000000000|MC|22102025|114016|7|500|0|597012345678`.

### 5.4 Otras respuestas

| Respuesta | Campos |
|:--|:--|
| `0810` cargar llaves | 2 `commerce_code`, 3 `terminal_id` |
| `0510` cierre | 2 `commerce_code`, 3 `terminal_id`, 4 `voucher` (opcional) |
| `0710` totales | 2 `tx_count` (entero), 3 `tx_total` (entero CLP) — ej. `0710\|00\|002\|15000\|\|` |
| `1210` anulación | 2 `commerce_code`, 3 `terminal_id`, 4 `authorization_code`, 5 `operation_number` — ej. `1210\|00\|597029414300\|IT750050\|162529\|000142\|\|` |

## 6. Mensajes intermedios `0900`

Solo llegan si la venta se envió con `status = 1`. Formato `0900|{code}`. `code` es un código §9 del rango 78–88 (ej. `78` "Esperando leer tarjeta", `81` "Solicitando ingreso de clave", `82` "Enviando transacción al host"). Un `code` desconocido o no numérico se entrega como `IntermediateStatus::Unknown(String)`, nunca es error.

## 7. POS Autoservicio

Mismo enlace y tramas (§2–§3) a **19200** baudios. Diferencias:

| Operación | Request | Respuesta |
|:--|:--|:--|
| Poll | `0100` | solo ACK |
| Cargar llaves | `0800` | `0810` |
| Inicialización | `0070` | solo ACK |
| Resultado de inicialización | `0080` | `1080` — campos 2 fecha `ddMMyyyy`, 3 hora `HHmmss`; código `90` = inicialización OK |
| Venta | `0200\|{amount}\|{ticket}\|{voucher}\|{status}` | `0210` (layout §7.1) |
| Venta multicódigo | `0270\|{amount}\|{ticket}\|{voucher}\|{status}\|{commerce_code}` | `0271` (§7.1 + 20 `commerce_provider_code`) |
| Última venta | `0250\|{voucher}` | `0260` (layout §7.1) |
| Anulación | `1200` | `1210` (anula la última venta) |
| Cierre | `0500\|{voucher}` | `0510` (4 = voucher) |

### 7.1 Venta Autoservicio

| # | Campo | # | Campo |
|:--|:--|:--|:--|
| 4 | `ticket` | 12 | `card_brand` |
| 5 | `authorization_code` | 13 | `real_date` |
| 6 | `amount` | 14 | `real_time` |
| 7 | `last4_digits` | 15 | `voucher` |
| 8 | `operation_number` | 16 | `installments_type` |
| 9 | `card_type` | 17 | `installments_number` |
| 10 | `accounting_date` | 18 | `installments_amount` |
| 11 | `account_number` | 19 | `installments_type_description` |

Ejemplo: `0210|00|597029414300|IM750164|123456|776549|10000|6590|58|CR|||VI|18032026|123506||03|03|3334|CUOTAS SIN INTERES`.

## 8. Recuperación ante fallas

| Situación | Qué hace la librería | Qué debe hacer la aplicación |
|:--|:--|:--|
| `NoAck` | Ya reintentó 3 veces | Revisar cable/puerto; `poll()` |
| `ResponseTimeout` en una venta | Devuelve el error; **no** reintenta la venta | **Nunca** reenviar la venta a ciegas: consultar `last_sale()` y comparar ticket y monto |
| Corte del puerto a mitad de la venta | `Io` | Igual que el anterior |
| `BadLrc` agotado | Error | `last_sale()` |
| Código `13`/`67`/`74`/`76`/`77` (debe ejecutar cierre) | Respuesta normal | Ejecutar `close()` |
| Código `26` (debe cargar llaves) | Respuesta normal | Ejecutar `load_keys()` |

La librería **nunca** reenvía por su cuenta una venta o una anulación después de recibido el ACK (riesgo de doble cobro). Solo reenvía tramas no acusadas (§3.1).

## 9. Códigos de respuesta

Del SDK Node (`responseCodes.js`). Mensajes en español tal como los entrega Transbank. `ResponseCode` es un enum con `Other(u16)` para códigos desconocidos.

| Código | Mensaje | Código | Mensaje |
|:--|:--|:--|:--|
| 0 | Aprobado | 25 | Fecha Invalida |
| 1 | Rechazado | 26 | Debe Cargar Llaves |
| 2 | Host no Responde | 27 | Debe Actualizar |
| 3 | Conexión Fallo | 60 | Error en Número de Cuotas |
| 4 | Transacción ya Fue Anulada | 61 | Error en Armado de Solicitud |
| 5 | No existe Transacción para Anular | 62 | Problema con el Pinpad interno |
| 6 | Tarjeta no Soportada | 65 | Error al Procesar la Respuesta del Host |
| 7 | Transacción Cancelada desde el POS | 67 | Superó Número Máximo de Ventas, Debe Ejecutar Cierre |
| 8 | No puede Anular Transacción Debito | 68 | Error Genérico, Falla al Ingresar Montos |
| 9 | Error Lectura Tarjeta | 70 | Error de formato Campo de Boleta MAX 6 |
| 10 | Monto menor al mínimo permitido | 71 | Error de Largo Campo de Impresión |
| 11 | No existe venta | 72 | Error de Monto Venta, Debe ser Mayor que 0 |
| 12 | Transacción No Soportada | 73 | Terminal ID no configurado |
| 13 | Debe ejecutar cierre | 74 | Debe Ejecutar CIERRE |
| 14 | No hay Tono | 75 | Comercio no tiene Tarjetas Configuradas |
| 15 | Archivo BITMAP.DAT no encontrado. Favor cargue | 76 | Supero Número Máximo de Ventas, Debe Ejecutar CIERRE |
| 16 | Error Formato Respuesta del HOST | 77 | Debe Ejecutar Cierre |
| 17 | Error en los 4 últimos dígitos. | 78 | Esperando Leer Tarjeta |
| 18 | Menú invalido | 79 | Solicitando Confirmar Monto |
| 19 | ERROR_TARJ_DIST | 80 | Selección de Cuotas |
| 20 | Tarjeta Invalida | 81 | Solicitando Ingreso de Clave |
| 21 | Anulación no Permitida | 82 | Enviando transacción al Host |
| 22 | TIMEOUT | 83 | Seleccion menu credito/redcompra |
| 24 | Impresora Sin Papel | 84 | Opere tarjeta |
| 90 | Inicialización OK (Autoservicio) | 85 | Seleccion de cuotas |
| 93 | Declinada | 86 | Ingreso de cuotas |
| 94 | Error al Procesar Respuesta | 87 | Confirmacion de cuotas |
| 95 | Error al Imprimir TASA | 88 | Error Cantidad Cuotas |

`is_approved()` es verdadero solo para `0` (y `90` en la inicialización de Autoservicio).

## 10. Preguntas abiertas (se cierran con un POS real — HW-01)

| ID | Pregunta | Decisión provisional |
|:--|:--|:--|
| OPEN-01 | Formato de la venta. Node/C: `0200\|000012000\|ABC123\|\|\|0` (monto con 9 dígitos, campo voucher vacío, sin `\|` final). .NET: `0200\|12000\|ABC123\|\|0\|1\|` (sin relleno, `\|` final) | §4: monto con 9 dígitos, campo voucher explícito, sin `\|` final. El simulador acepta las tres variantes |
| OPEN-02 | ¿Se acusa con ACK cada `0900`? Node sí; .NET no | Sí (§3.2) |
| OPEN-03 | ¿Cuántas tramas vacías cierran el detalle? | Ventana `detail_tail` (§4.2) |
| OPEN-04 | ¿El POS acepta el ticket en minúsculas? Los ejemplos usan `abc123` | Se envía tal como lo da el usuario |
| OPEN-05 | ¿Llegan bytes basura al abrir el puerto USB? | El decodificador los descarta (§2.4) |
| OPEN-06 | Layout del voucher de cierre y de última venta en Integrado | Se expone crudo |
