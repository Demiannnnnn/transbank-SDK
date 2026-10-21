# Orquestación (Claude Code + agy)

Herramientas del orquestador para delegar tareas a `agy` (Antigravity CLI) y validar su trabajo. No forman parte del SDK. Mismo método que los otros repos del dueño (adaptado de `telemetry-apps`, rama `develop-AGENTS`, `tools/orchestration/`). El rol de Claude está en [`CLAUDE.md`](../../CLAUDE.md).

## Worktrees

| Worktree (junto al repo) | Rama | Uso |
|:--|:--|:--|
| `../transbank-SDK` | `main` | Checkout del dueño: no se toca |
| `../tbk-wt-orch` | `develop-AGENTS` | Orquestador: docs, merges, push |
| `../tbk-wt-pro`, `../tbk-wt-plus` | `dev/*` | "Slots" de agy: un agente por worktree (lock `/tmp/agy-<worktree>.lock`, exit 75 = BUSY) |

Crear los slots: `git worktree add --detach ../tbk-wt-pro develop-AGENTS` (ídem `plus`). Borra `target/` de un slot cuando su tarea se integra (disco).

## Scripts

| Script | Qué hace |
|:--|:--|
| `run_agy.sh <tasks/…md> <dev/rama> [modelo] [notas]` | Lanza agy en el slot `AGY_SLOT` (por defecto `pro`) sobre una rama `dev/*` creada desde `develop-AGENTS`. Logs en `~/.cache/tbk-orchestration/logs`. Modelo por defecto `gemini-3.8-flash-high` |
| `run_agy_retry.sh` | Igual, con reintentos ante cortes de red; cuota agotada ⇒ exit 3 `QUOTA_EXHAUSTED` |
| `validate.sh` | `REPO_DIR=<slot>`: diff contra `develop-AGENTS`, archivos protegidos, dependencias nuevas, trampas, y los cinco comandos de AGENTS.md §8 |
| `review_agy.sh <slot> <tarea> [modelo]` | Revisión de solo lectura por agy en un worktree desechable (por defecto `gemini-4-argon-high`) ⇒ `VERDICT: PASS/FAIL` |
| `pipeline.sh <orch-dir> <slot> <cola>` | Cola completa en shell: implementa → valida → revisa → una ronda de arreglos → resumen en `~/.cache/tbk-orchestration/summary/tbk-<tarea>.md` (`STATUS: READY` / `NEEDS_ORCHESTRATOR`) |
| `usage.sh` | Cuota restante de agy |
| `agy-note.txt` | Notas de entorno que se agregan al prompt de agy |

Línea de cola: `<tasks/…md> <dev/rama> [modelo] [resume|-] [notas-extra]`. Ejemplo (`~/.cache/tbk-orchestration/queue-pro.txt`):

```
tasks/infra/task-01-workspace-and-ci.md dev/infra-01-workspace
tasks/proto/task-01-framing.md dev/proto-01-framing
```

```bash
cd ../tbk-wt-orch
nohup tools/orchestration/pipeline.sh . pro ~/.cache/tbk-orchestration/queue-pro.txt > ~/.cache/tbk-orchestration/pipeline-pro.log 2>&1 &
```

Las tareas del núcleo son **secuenciales** (`tasks/README.md` §3): una cola en un solo slot. El segundo slot sirve para SERIAL-01 ∥ AUTO-01 y ASYNC-01.

## Flujo

1. Claude escribe/actualiza docs y tareas en una rama `dev/docs-*` desde `tbk-wt-orch` y las integra a `develop-AGENTS`.
2. `usage.sh`; arma la cola y lanza `pipeline.sh` en segundo plano.
3. Lee los resúmenes. `READY` ⇒ mirada rápida al diff (bytes contra la spec, LINK-03, tests reales) y merge. `NEEDS_ORCHESTRATOR` ⇒ notas + relanzar en la misma rama.
4. Merge aceptado: `git merge --no-ff dev/<rama>` en `tbk-wt-orch`, actualizar `tasks/README.md` (estado ✅) y `.ai-context/implementation-status.md`, push de `develop-AGENTS`.

## Reglas

- `main` y `develop` nunca se tocan; `develop-AGENTS` solo el orquestador; agy solo `dev/*`.
- Un agente por worktree; los comandos `cargo` pesados se serializan con el wrapper de `~/.cache/telemetry-orchestration/bin/cargo` si existe (instalado por `telemetry-apps/tools/orchestration/native/setup-native.sh`); si no, `CARGO_BUILD_JOBS=2`.
- Nadie publica en crates.io ni crea tags (`docs/release.md`).
- Si la cuota Gemini está en 0 %, Claude implementa (CLAUDE.md), con el mismo flujo de rama `dev/*`, `validate.sh` y merge.

## Preparar un equipo

1. Rust estable ≥ 1.85 con clippy y rustfmt (Linux: `libudev-dev` o equivalente).
2. `agy` instalado y con sesión (`agy-login` o `agy` interactivo una vez, lo hace el dueño).
3. Clonar y crear los worktrees (`.ai-context/handoff.md` §3).
4. Opcional: wrappers de `telemetry-apps` (`native/setup-native.sh`) para serializar `cargo` entre repos.
