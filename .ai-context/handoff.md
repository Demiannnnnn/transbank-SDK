# Handoff — transbank-SDK

Última actualización: 2026-10-21 — documentación base, spec del protocolo y plan de tareas integrados en `develop-AGENTS`. Nada en curso. Siguiente: lanzar INFRA-01 con agy.

## 1. Estado

- Repo creado por el dueño (`Demiannnnnn/transbank-SDK`, commit inicial en `main`). Ramas `develop` y `develop-AGENTS` creadas y publicadas el 2026-10-21; desde entonces el orquestador solo escribe en `develop-AGENTS` y `dev/*`.
- `develop-AGENTS` tiene: `AGENTS.md`, `CLAUDE.md`, `README.md`, licencias (`LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`), `.ai-context/` completo, `docs/` (protocolo, simulador, hardware, publicación), `tasks/` (12 tareas) y `tools/orchestration/`.
- Sin código. Orden: `tasks/README.md` §3.

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

1. Integrar `develop-AGENTS → develop` cuando quiera.
2. HW-01 cuando exista la CLI: probar con su POS Integrado (`docs/hardware-testing.md`).
3. Autorizar REL-01 (crates.io) tras HW-01.

## 6. Lo que no está en el repo

| Qué | Dónde | Cómo reconstruirlo |
|:--|:--|:--|
| Clones de los SDKs oficiales usados para la spec | scratchpad de la sesión del 2026-10-21 (borrado) | `git clone --depth 1 https://github.com/TransbankDevelopers/transbank-pos-sdk-{nodejs,dotnet,c}` |
| Logs y resúmenes de agy | `~/.cache/tbk-orchestration/` | No se necesitan |
| Memoria de Claude Code | `~/.claude/projects/.../memory/` | Este archivo la reemplaza |
