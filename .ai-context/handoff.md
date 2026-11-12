# Handoff — transbank-SDK

Última actualización: 2026-11-12 — auditoría del protocolo integrada (`docs/release-prep-2026-11.md`: TBK-01/03/04/07 corregidos, TBK-05/06 pendientes), 405 tests; propuesta del código 90 en PR #1 (borrador, decide el dueño); PR `develop-AGENTS → develop` abierto. Sin publicar ni tag. Antes: 2026-11-05 — AUTO-01 y ASYNC-01 integradas en `develop-AGENTS`, 396 tests, sin publicar ni tag. Nada en curso. HW-01 sigue siendo del dueño. Worktree de integración: `~/wt/tbk-int` (reemplaza a `../tbk-wt-orch`).

## 1. Estado

- Repo creado por el dueño (`Demiannnnnn/transbank-SDK`, commit inicial en `main`). Ramas `develop` y `develop-AGENTS` creadas y publicadas el 2026-10-21; desde entonces el orquestador solo escribe en `develop-AGENTS` y `dev/*`.
- `develop-AGENTS` tiene: `AGENTS.md`, `CLAUDE.md`, `README.md`, licencias (`LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`), `.ai-context/` completo, `docs/` (protocolo, simulador, hardware, publicación), `tasks/` (12 tareas) y `tools/orchestration/`.
- Código: workspace de 3 crates (`transbank-pos`, `transbank-pos-sim`, `transbank-pos-cli`), 271 tests (TEST-01: propiedades, `serde`, enlace, modelo, CLI; `cargo-mutants` sin escapes), los cinco comandos de AGENTS.md §8 en verde en macOS. Cada tarea tiene su Reporte con decisiones y desviaciones; resumen en `implementation-status.md`.
- Ramas `dev/*` integradas (y publicadas): `infra-01-workspace`, `proto-01-framing`, `proto-02-integrado-messages`, `link-01-transport-and-link`, `sim-01-simulator`, `client-01-pos-integrado`, `serial-01-serial-and-tcp`, `cli-01-tbk-pos`, `docs-01-release-prep`, `test-01-hardening`. Se pueden borrar cuando el dueño integre `develop-AGENTS → develop`.
- Worktree `../tbk-wt-pro` existe (último uso: DOCS-01); `../tbk-wt-plus` no se creó.

## 2. Origen y decisiones del dueño

- **2026-10-03** (en `rework-sistema-facturacion`, `.ai-context/handoff.md` §6.1, tarea FUT-01): la integración con Transbank POS Integrado se hace en Rust como módulo **genérico y autocontenido** (protocolo serial + transporte + simulador, sin tipos de ese proyecto), en un **repo propio**, licencia dual MIT/Apache-2.0, citando los SDKs oficiales (BSD-3-Clause) y declarando que es **no oficial**. **No publicar sin validar con un POS real en modo integrado.** Objetivo: portafolio, no negocio.
- **2026-10-21**: el dueño creó este repo y pidió construirlo con la misma estructura que `rework-sistema-facturacion` (`.ai-context/`, `AGENTS.md`, `tasks/`, README). Reglas de ramas: solo `develop-AGENTS` y ramas propias `dev/*`; `main` y `develop` no se tocan.
- Decisiones de diseño del orquestador (ADR-01…11 en `architecture.md`): núcleo sans-IO, API bloqueante primero, rechazo ≠ error, sin reintento de ventas tras el ACK, simulador como crate propio.

## 3. Cómo retomar en otro equipo

```bash
git clone https://github.com/Demiannnnnn/transbank-SDK.git
cd transbank-SDK
git worktree add ../tbk-wt-orch develop-AGENTS          # orquestador
git worktree add --detach ../tbk-wt-pro develop-AGENTS  # slot agy 1
git worktree add --detach ../tbk-wt-plus develop-AGENTS # slot agy 2
```

Requisitos: Rust estable ≥ 1.85 con clippy y rustfmt (en el Mac del dueño: 1.99.1 de Homebrew); Linux: `libudev-dev`. `agy` con sesión iniciada (ver `tools/orchestration/README.md`).

## 4. Forma de trabajo

Igual que en los otros repos del dueño (`CLAUDE.md`): Claude orquesta y no escribe código del producto; agy implementa con `tools/orchestration/pipeline.sh`; Claude lee los resúmenes, valida con `validate.sh`, revisa el diff e integra con `merge --no-ff` en `develop-AGENTS`. Si la cuota Gemini de agy está en 0 %, Claude implementa hasta agotar la suya.

## 5. Pendientes del dueño

0. Leer `docs/release-prep-2026-11.md` (versión recomendada 1.1.0, checklist HW-01/REL-01) y decidir PR #1 (`is_approved()` y el código 90). Revisar lo marcado en AUTO-01: OPEN-07 y OPEN-08 de `protocol-spec.md`; `SaleResponse::is_approved()` de Integrado acepta el código 90 (`implementation-status.md`); `CHANGELOG.md` [Unreleased] queda sin versión hasta que el dueño publique. Con las APIs nuevas, `transbank-pos-sim` y la CLI dependen de una `transbank-pos` posterior a la 1.0.0: al publicar hay que subir la versión del workspace y sus dependencias internas (minor). Desde ASYNC-01, `Link` usa `exchange.rs`: es un refactor interno de la 1.0.0, revisado y con la batería anterior intacta.
1. Integrar `develop-AGENTS → develop` cuando quiera (dispara el CI en Linux/macOS/Windows).
2. HW-01: probar con su POS Integrado siguiendo `docs/hardware-testing.md` (`cargo build -p transbank-pos-cli --release`).
3. Opcional: publicar `transbank-pos 0.0.1` para reservar el nombre (`docs/release.md` §0; ya cumple el mínimo).
4. Autorizar REL-01 (0.1.0 en crates.io) tras HW-01.

## 6. Lo que no está en el repo

| Qué | Dónde | Cómo reconstruirlo |
|:--|:--|:--|
| Clones de los SDKs oficiales usados para la spec | scratchpad de la sesión del 2026-10-21 (borrado) | `git clone --depth 1 https://github.com/TransbankDevelopers/transbank-pos-sdk-{nodejs,dotnet,c}` |
| Logs y resúmenes de agy | `~/.cache/tbk-orchestration/` | No se necesitan |
| Memoria de Claude Code | `~/.claude/projects/.../memory/` | Este archivo la reemplaza |
