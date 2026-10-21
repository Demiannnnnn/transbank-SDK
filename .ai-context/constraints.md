# Restricciones — transbank-SDK

Reglas duras. Violar una es un **defecto bloqueante**, aunque los tests pasen. Las tareas las referencian por ID. `AGENTS.md` (reglas de proceso) tiene prioridad sobre este archivo; este archivo tiene prioridad sobre las tareas.

## PROTO — protocolo (sans-IO)

| ID | Regla |
|:--|:--|
| **PROTO-01** | **Bytes exactos de `docs/protocol-spec.md`.** Cada request y los vectores de §2.3 tienen un test que compara la trama completa byte a byte. |
| **PROTO-02** | **LRC = XOR desde el primer byte del payload hasta ETX inclusive.** Una sola implementación (`protocol::lrc`), usada por el codificador, el decodificador y el simulador. |
| **PROTO-03** | **El decodificador no interpreta el byte de LRC** (puede ser `0x02`, `0x03`, `0x06`, `0x15`). Tests con los vectores `0100` (LRC `0x02`) y `0500\|\|` (LRC `0x06`). |
| **PROTO-04** | **Nunca entra en pánico** con ningún input: `proptest` con bytes arbitrarios y trozos arbitrarios. Payload máximo 4096 bytes (`Overflow`). |
| **PROTO-05** | **Parseo tolerante y honesto:** campos ausentes o vacíos ⇒ `None`; un número no vacío inválido ⇒ `Error::Parse` con el nombre del campo. Nunca se inventan valores por defecto (`0`, `""`) para campos ausentes. |
| **PROTO-06** | **Se conserva el payload crudo** en cada respuesta (`raw`) y los ceros a la izquierda de `last4_digits`, `ticket`, `authorization_code`, `operation_number` como texto donde la spec lo indique. |
| **PROTO-07** | **Validación antes de enviar** (`protocol-spec.md` §4.1): monto, ticket, operación, código de comercio. Nada inválido llega al puerto. |
| **PROTO-08** | **Una tabla por terminal:** los layouts de Integrado y Autoservicio son tipos distintos; no se comparte un parser con `if` por modelo. |

## LINK — enlace

| ID | Regla |
|:--|:--|
| **LINK-01** | **ACK tras cada trama recibida con LRC válido**, antes de parsearla; **NAK** ante LRC inválido (máx. 3 seguidos ⇒ `Error::BadLrc`). |
| **LINK-02** | **Reintento solo de tramas no acusadas:** máx. `max_attempts` (3) envíos de la misma trama si llega NAK o vence `ack_timeout`. |
| **LINK-03** | **Nunca reenviar una venta, venta multicódigo ni anulación después del ACK** (ADR-04). Test que lo demuestra con un POS simulado que no responde. |
| **LINK-04** | **Timeouts configurables** con defaults de la spec: `ack_timeout` 2 s, `response_timeout` 150 s, `detail_tail` 500 ms. El plazo de respuesta no se reinicia con mensajes `0900`. |
| **LINK-05** | **Una operación a la vez:** los métodos toman `&mut self`; no hay estado global ni `static mut`. |
| **LINK-06** | **Código de función inesperado ⇒ `Error::UnexpectedResponse`** con esperado, recibido y payload. Nunca se parsea una respuesta como otra. |
| **LINK-07** | **Basura descartada:** bytes fuera de trama al esperar ACK o respuesta se descartan (y se trazan), no abortan la operación. |

## API — API pública

| ID | Regla |
|:--|:--|
| **API-01** | **La API pública es la de `public-api.md`.** Agregar, quitar o cambiar un ítem público requiere que la tarea lo diga. |
| **API-02** | **Rechazo ≠ `Err`** (ADR-03). `is_approved()` en cada respuesta. |
| **API-03** | **Montos CLP como enteros** (`u32`/`u64`). Prohibido `f32`/`f64` en todo el workspace. |
| **API-04** | **Newtypes validados** (`Amount`, `Ticket`, `OperationNumber`, `CommerceCode`) con constructores `TryFrom`/`new` que devuelven `Result`. |
| **API-05** | **Enums públicos `#[non_exhaustive]`** (`Error`, `ResponseCode`, `CardType`, `CardBrand`, `IntermediateStatus`…) y structs de respuesta `#[non_exhaustive]` con campos públicos. |
| **API-06** | **Todo ítem público documentado** (`missing_docs` = deny) con al menos un ejemplo en los tipos y métodos principales (doctests que compilan; los que necesitan hardware usan `no_run` con el simulador o `MemoryTransport`). |
| **API-07** | **Sin pánicos en la API** (AGENTS.md §6) y `Send` en clientes y transportes (test de compilación `fn assert_send<T: Send>()`). |
| **API-08** | **Features aditivas:** `--no-default-features` compila (sin `serial`); `serde` y `tracing` no cambian comportamiento. |

## SAFE — robustez

| ID | Regla |
|:--|:--|
| **SAFE-01** | `#![forbid(unsafe_code)]` en cada crate (también vía `[workspace.lints]`). |
| **SAFE-02** | Buffers acotados: payload ≤ 4096, detalle ≤ 10 000 ventas (`Error::TooManyFrames`), líneas de la CLI acotadas. |
| **SAFE-03** | El transporte serie cierra el puerto al hacer `drop`. Sin hilos sueltos: nada de `thread::spawn` sin `join` en la librería. |

## SEC — seguridad y privacidad

| ID | Regla |
|:--|:--|
| **SEC-01** | **Sin datos de tarjeta completos:** el SDK nunca pide ni guarda PAN, CVV ni clave (el POS no los entrega). |
| **SEC-02** | **Trazas sin datos sensibles** (`security-model.md` §3): payloads completos solo en nivel `trace` y con `account_number` y voucher redactados. |
| **SEC-03** | **`Debug` de respuestas redacta** `account_number` y el voucher (muestra largo, no contenido). |
| **SEC-04** | **Simulador y `TcpTransport` del simulador escuchan solo en `127.0.0.1` por defecto**; escuchar en otra interfaz exige un flag explícito y un aviso. |

## LEG — licencias y marca

| ID | Regla |
|:--|:--|
| **LEG-01** | Licencia `MIT OR Apache-2.0` en cada `Cargo.toml`; `NOTICE` incluido en cada crate publicado (`include`). |
| **LEG-02** | Sin código copiado de los SDKs oficiales ni de otros proyectos. Datos portados (mensajes de §9, tramas de ejemplo) con comentario de origen. |
| **LEG-03** | "No oficial" visible: README, docs de cada crate (`//!`), `--help` de la CLI y del simulador. Sin logos de Transbank. |
| **LEG-04** | Nombres de crates sin dar a entender que son oficiales (`transbank-pos` + descripción "Unofficial…"). |

## QA — calidad

| ID | Regla |
|:--|:--|
| **QA-01** | Cada RF de una tarea tiene tests. Cada respuesta de §5 y §7 de la spec tiene un test con el ejemplo real del SDK (fixtures). |
| **QA-02** | Tests de integración cliente ↔ simulador para cada operación pública: aprobada, rechazada, timeout, NAK, LRC corrupto. |
| **QA-03** | **Tests deterministas:** sin `thread::sleep`, sin puertos serie reales, sin red externa. Timeouts con reloj manual; TCP solo en `127.0.0.1:0`. |
| **QA-04** | `cargo test` completo en menos de 60 s en un equipo normal. |
