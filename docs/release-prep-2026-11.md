# Preparación de release y auditoría del protocolo — 2026-11

Documento para que el dueño decida la próxima publicación. **No se creó ningún tag ni se publicó nada.** Base: `develop-AGENTS` en `2f224ee` más la rama `dev/audit-protocol`.

## 1. Estado publicado

- crates.io: `transbank-pos`, `transbank-pos-sim` y `transbank-pos-cli` **1.0.0** (verificado con `cargo search` el 2026-11-12). Sin tags en GitHub.
- Desde la 1.0.0, `develop-AGENTS` agregó (CHANGELOG `[Unreleased]`): cliente POS Autoservicio (AUTO-01), API async con tokio detrás de features (ASYNC-01), escenarios Autoservicio del simulador, opciones nuevas en la CLI y los arreglos de §3.

## 2. Qué versión subir y por qué

**Recomendación: 1.1.0 para los tres crates** (versión del workspace y dependencias internas `version = "1.1.0"`).

- Lo nuevo es aditivo: tipos, métodos y features nuevos sin quitar ni cambiar firmas de la 1.0.0 → *minor* según SemVer (para la regla `1.x`, agregar API pública es minor).
- `transbank-pos-sim` y `transbank-pos-cli` usan APIs nuevas de `transbank-pos`: no pueden publicarse contra la 1.0.0. Hay que publicar los tres con la misma versión, en orden `transbank-pos` → `transbank-pos-sim` → `transbank-pos-cli` (`docs/release.md` §2).
- Los arreglos de §3 cambian comportamiento observable solo ante fallas de línea (ruido, LRC malos, `0900` repetidos) y lo alinean con `protocol-spec.md`: caben en una minor (y también en un patch si se publicaran solos).
- Propuesta pendiente del dueño (PR #1, borrador, `dev/fix-sale-approved-proposal`): `is_approved()` estricto con el código 90 en POS Integrado. Si se acepta, entra en la 1.1.0 con su nota de compatibilidad en el CHANGELOG; si no, la 1.1.0 sale igual y el tema queda documentado como limitación.
- No se recomienda 2.0.0: no hay cambios incompatibles de API.

## 3. Auditoría del protocolo (framing, LRC, timeouts, reintentos, parsing)

Revisor: agy con `gemini-4-argon-high` (segunda opinión), contra `docs/protocol-spec.md`. Cada hallazgo se verificó leyendo el código; los corregidos tienen primero un test que fallaba con el código original. Commits en `dev/audit-protocol`.

| ID | Severidad | Tema | Estado |
|:--|:--|:--|:--|
| TBK-01 | Alta | Un `STX` suelto en un intento fallido dejaba el decodificador dentro de una trama y el ACK del reintento se tragaba como payload: la trama se reenviaba **después** de ser acusada (contra LINK-03) y terminaba en `NoAck` falso | Corregido (`50b9cf9`): `decoder.reset()` antes de cada reintento (`Link` y `AsyncLink`) |
| TBK-03 | Media | En comandos sin respuesta (`poll`, `0300`, `0070`, `0260\|0\|`) una trama o un BadLrc contaban como ACK implícito: ruido u otro equipo serie hacían que `poll()`/`autoconnect` dieran éxito sin POS | Corregido (`f502258`): solo el ACK cierra esos comandos; el ACK implícito se mantiene en `request`/`request_list` (no reenviar una venta) |
| TBK-04 | Media | La cola del detalle de ventas respondía `NAK` a LRC malos sin límite | Corregido (`1d70606`): `Error::BadLrc` tras más de `max_bad_lrc` seguidos; los eventos ignorados no renuevan `detail_tail` |
| TBK-07 (1) | Baja | En `request_list` cada `0900` reiniciaba `response_timeout` (§3.2 paso 4 dice que no) | Corregido (`2cbcac2`) |
| TBK-05 | Baja | Al superar `MAX_PAYLOAD` (4096) el decodificador vuelve a `Idle` antes del `ETX`+LRC de esa trama; si el LRC vale `0x06`/`0x15`/`0x02` se interpreta como ACK/NAK/STX. La tabla de §2.4 describe justamente ese comportamiento | Pendiente: requiere un estado "descartar hasta ETX+LRC" y cambiar la spec; tramas >4 KiB no existen en el protocolo conocido |
| TBK-06 | Baja | `Link::new`/`AsyncLink::new` usan el layout de trazas de Integrado: con tramas Autoservicio en un enlace creado a mano (no vía `PosAutoservicio`), las trazas `trace` muestran `account_number` y voucher sin redactar. También faltan trazas de la cola del detalle y de la basura descartada (LINK-07) | Pendiente: redactar la unión de campos sensibles cuando el layout no está fijado |
| TBK-02 | — | "EOF del puerto serie devuelve `Ok(0)` y gira al 100 % de CPU" | No se reproduce: `serialport` 4.10 en Linux devuelve `BrokenPipe` ante `POLLHUP` y en Windows convierte la lectura vacía en `TimedOut` |
| TBK-07 (2) | Info | `detail_tail` acota cada evento completo, no "500 ms sin bytes" | Sin cambio: solo importa a baudios muy bajos; se cierra con HW-01 |

Parsing de respuestas, LRC, codificación ISO-8859-1, overflow numérico y transportes TCP/memoria: sin hallazgos.

## 4. Checklist antes de publicar

### HW-01 (lo hace el dueño, `docs/hardware-testing.md`)

- [ ] `cargo build -p transbank-pos-cli --release` y conectar el POS en modo integrado.
- [ ] `tbk-pos poll`, `load-keys`, venta aprobada y rechazada, `last-sale`, `refund`, `totals`, `sales-detail` (impreso y enviado), `close`.
- [ ] Responder OPEN-01…08 de `protocol-spec.md` §10 (formato de venta, ACK de cada `0900`, tramas vacías del detalle, ticket en minúsculas, basura al abrir el puerto, vouchers, relleno Autoservicio, código de éxito de `1080`).
- [ ] Si el POS devuelve alguna vez el código 90 en una operación Integrado, anotarlo para la decisión de PR #1.

### REL-01 (publicación 1.1.0, la hace el dueño)

- [ ] Decidir PR #1 (código 90) y, si se acepta, integrarlo en `develop-AGENTS`.
- [ ] Integrar `develop-AGENTS → develop` (PR) con el CI multiplataforma en verde (Linux, macOS, Windows), y luego `develop → main`.
- [ ] Subir la versión del workspace a `1.1.0` (y `version` de las dependencias internas en `Cargo.toml` raíz); en `CHANGELOG.md` renombrar `[Unreleased]` a `[1.1.0] - <fecha>` y ajustar su frase "Not released".
- [ ] AGENTS.md §8 en verde en local.
- [ ] `cargo publish --dry-run -p transbank-pos` (y luego `-sim`, `-cli`) sin advertencias.
- [ ] Publicar en orden `transbank-pos` → `transbank-pos-sim` → `transbank-pos-cli`; tag `v1.1.0` y release de GitHub con el CHANGELOG.
- [ ] README: mantener el aviso "unofficial" y "never tested against a real terminal" mientras HW-01 no esté hecho.

## 5. Decisiones del dueño

1. PR #1: `is_approved()` estricto con el código 90 (recomendado: aceptarlo).
2. Publicar 1.1.0 antes o después de HW-01 (la 1.0.0 se publicó sin HW-01 por decisión del 2026-10-22).
3. TBK-05 y TBK-06: arreglarlos antes de la 1.1.0 (recomendado TBK-06, por ser una fuga de datos en trazas, aunque solo en nivel `trace` y con enlaces creados a mano).
