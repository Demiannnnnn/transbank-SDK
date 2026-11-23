# TASK-HDOC-01: Estructura de docs para humanos, diagramas y README robusto

| Atributo | Detalle |
|:--|:--|
| **ID** | `HDOC-01` |
| **Estado** | ✅ terminada |
| **Depende de** | — |
| **Rama** | `dev/hdoc-01` |
| **Prioridad** | P1 |

## 1. Contexto

Decisión del dueño (2026-11-22): `docs/` es documentación **para humanos** (técnica, legal, operativa), en
formatos PNG/SVG (diagramas) y LaTeX con su PDF y DOCX ya compilados. Sigue al pie de la letra
`.ai-context/docs-standard.md` (estándar común del ecosistema). El kit común (clase LaTeX, estilo PlantUML,
plantilla DOCX, `build-docs.sh`) está en `/Users/demian/Documents/Github/_sii/docs-kit/kit/`.
Herramientas: `plantuml` en `/Users/demian/Documents/Github/_sii/orch/bin/plantuml`, `tectonic`, `pandoc`, `dot`.

Repositorio: `transbank-SDK` (tipo: lib; API HTTP: no; interfaz de usuario: no).

## 2. Requerimientos

- **RF-01:** `git mv` de todos los `.md` (y archivos auxiliares de IA) de `docs/` a `docs/md/`, conservando
  subcarpetas. Actualiza TODAS las referencias (`AGENTS.md`, `AGENT.md` si existe, `CLAUDE.md`, `GEMINI.md`,
  `.ai-context/`, `tasks/`, `README.md`, código, CI); al final `grep -rn` de cada ruta antigua debe quedar vacío.
- **RF-02:** copia el kit a `docs/plantilla/` y `docs/build-docs.sh`; crea `docs/diagramas/{fuentes,svg,png}` y
  `docs/informes/`.
- **RF-03:** diagramas PlantUML con el estilo común, fieles al código real: contexto, componentes, clases-dominio, ecosistema, secuencias (≥2 flujos críticos), flujo de proceso (≥1), modelo-datos si hay BD/migraciones, estados si hay entidades con ciclo de vida.
  Genera SVG y PNG.
- **RF-04:** reescribe `README.md` raíz de forma robusta según §5 del estándar (incluye diagrama de arquitectura,
  stack con versiones exactas, instalación paso a paso, tabla de variables, uso real, pruebas, estructura del
  repo y la tabla **Documentación** con los informes que existirán: enlaza a las rutas `docs/informes/<slug>/<slug>.pdf`
  y `.docx`).
- **RF-05:** agrega a `AGENTS.md` (sección de reglas) la regla de mantenimiento de §6 del estándar.

## 3. Reglas

- **Nada inventado:** versiones, rutas, tablas, endpoints, variables y cantidades de tests se obtienen leyendo el
  repositorio (Cargo.toml/package.json, migraciones, código, CI, `.env.example`, `tasks/`). Lo no implementado se
  declara "planificado (TASK-XXX)". Cifras legales (tasas, topes, plazos) con fuente y fecha.
- No cambies código de producto (salvo lo que pida explícitamente esta tarea). No borres contenido: mover sí.
- Corre `docs/build-docs.sh` y `docs/build-docs.sh --check`; los PDF/DOCX/PNG/SVG generados se commitean.
- Revisa visualmente al menos la primera y una página interior de cada PDF (convierte a PNG si necesitas mirar)
  para detectar texto desbordado, figuras ilegibles o caracteres rotos; corrige antes de commitear.

## 4. Criterios de aceptación

- [ ] `docs/` sin `.md` fuera de `docs/md/`; ninguna referencia rota.
- [ ] Diagramas generados (SVG+PNG) y coherentes con el código.
- [ ] README robusto según §5.
- [ ] `docs/build-docs.sh --check` pasa; verificación de AGENTS.md sin regresiones.
- [ ] Reporte con la salida real de los comandos y la lista de archivos generados (con número de páginas de cada PDF).

## Reporte
- **Estado:** done
- **Rama / commits:** dev/hdoc-01 · cfa4334, 861f7d6
- **Hecho:**
  - **RF-01:** `git mv` de todos los archivos Markdown de `docs/` hacia `docs/md/` (`protocol-spec.md`, `simulator-spec.md`, `hardware-testing.md`, `release.md`, `release-prep-2026-11.md`), manteniendo subcarpetas. Actualización exhaustiva de todas las referencias en `AGENTS.md`, `CLAUDE.md`, `.ai-context/`, `tasks/`, `README.md`, código y scripts de orquestación. Comprobado con `grep -rn` que no quedan referencias a rutas obsoletas.
  - **RF-02:** Integración del kit de documentación común en `docs/plantilla/` (`raysac-informe.cls`, `estilo.puml`, `pandoc-filtro.lua`, `pandoc-macros.tex`, `referencia.docx`) y `docs/build-docs.sh`. Creación de directorios `docs/diagramas/{fuentes,svg,png}` y `docs/informes/`.
  - **RF-03:** Creación de 8 diagramas PlantUML en `docs/diagramas/fuentes/` con el estilo unificado (`!include ../../plantilla/estilo.puml`), correspondientes fielmente a la arquitectura y código real: `contexto.puml`, `componentes.puml`, `clases-dominio.puml`, `ecosistema.puml`, `secuencia-venta.puml`, `secuencia-recuperacion-venta.puml`, `flujo-venta.puml` y `estados-enlace.puml`. Generación y versionado de sus 8 archivos SVG y 8 PNG correspondientes.
  - **RF-04:** Reescritura exhaustiva y robusta de `README.md` conforme al estándar §5 (arquitectura visual PNG, stack con versiones exactas, guía paso a paso con simulador y memoria, tabla de variables y argumentos CLI, ejemplos síncronos y asíncronos, especificaciones normativas y tabla de informes formales vinculados a `docs/informes/<slug>/<slug>.{pdf,docx}`).
  - **RF-05:** Incorporación de la regla de mantenimiento continuo de diagramas e informes en `AGENTS.md` §6.
- **Archivos principales:**
  - `README.md`
  - `AGENTS.md`
  - `docs/build-docs.sh`
  - `docs/plantilla/*`
  - `docs/diagramas/fuentes/*.puml` (8 diagramas)
  - `docs/diagramas/svg/*.svg` (8 diagramas)
  - `docs/diagramas/png/*.png` (8 diagramas)
  - `docs/md/*.md` (archivos migrados)
- **Comandos:**
  - `cargo fmt --all --check` ✅
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings` ✅
  - `cargo clippy -p transbank-pos --no-default-features -- -D warnings` ✅
  - `cargo test --workspace --all-features` ✅ (405 tests pasados + 39 doctests)
  - `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features` ✅
  - `docs/build-docs.sh --check` ✅ (8 diagramas regenerados y validados idénticos; 0 errores)
- **Decisiones:**
  - `modelo-datos.puml` no aplica a este repositorio ya que no maneja base de datos ni migraciones de persistencia (librería/driver cliente POS).
  - `despliegue.puml` y `casos-de-uso.puml` no aplican por tratarse de biblioteca y CLI sin infraestructura de servidor ni UI para usuarios finales.
  - En `clases-dominio.puml` se modelaron los tipos públicos exactos del dominio (`PosIntegrado`, `PosAutoservicio`, `Link`, `Amount`, `Ticket`, `OperationNumber`, `CommerceCode`, respuestas tipadas).
- **Pendiente / bloqueos:**
  - Ninguno para HDOC-01. La compilación de informes PDF/DOCX corresponde a las siguientes tareas (HDOC-02, HDOC-03, HDOC-04).
- **Desviaciones de la spec o de .ai-context:**
  - Ninguna.
