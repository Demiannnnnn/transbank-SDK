# Tarea DOCS-01: Rustdoc, ejemplos, READMEs de crates, CHANGELOG y `publish --dry-run`

| Atributo | Detalle |
|:--|:--|
| **ID** | `TASK-DOCS-01` |
| **Estado** | ✅ hecho |
| **Rama** | `dev/docs-01-release-prep` |
| **Depende de** | CLI-01 |
| **Restricciones** | API-06, LEG-01…04 · `docs/release.md` §1 · `conventions.md` §1, §7 |
| **Prioridad** | P1 |

## 1. Contexto

Dejar los tres crates listos para publicar (sin publicar: eso es REL-01, del dueño). La documentación de docs.rs es la cara del proyecto en el portafolio del dueño.

## 2. Requerimientos

### RF-01 Rustdoc
- Página de crate de `transbank-pos` (`//!`): qué es, aviso unofficial, modelos soportados, quick start con el simulador, tabla de operaciones, sección "Avoiding double charges" (ADR-04), features, MSRV, licencia.
- Revisar que cada ítem público tenga docs y `# Errors`; ejemplos en `PosIntegrado`, `SaleRequest`, `ResponseCode`, `MemoryTransport`, `SimTransport`.
- `#![doc(html_root_url)]` no; `[package.metadata.docs.rs] all-features = true`.

### RF-02 READMEs de crates (inglés)
`crates/*/README.md` completos (instalación, ejemplo, enlace al repo y a la spec, aviso unofficial). El README raíz (español) solo se ajusta si algo de lo que dice ya no es cierto — **reporta** el cambio (es archivo del orquestador; cambio mínimo permitido aquí).

### RF-03 `CHANGELOG.md`
Formato Keep a Changelog, sección `[Unreleased]` con todo lo de v0.1.0.

### RF-04 Empaquetado
- Metadatos de `docs/release.md` §1 en cada `Cargo.toml`.
- `NOTICE` y licencias dentro de cada paquete: si `include` con rutas `../../` no funciona, usar `license-file` no aplica (es dual); la solución aceptada es un `build`-free: copiar `LICENSE-MIT`, `LICENSE-APACHE` y `NOTICE` dentro de cada crate **como archivos idénticos** y un test (`tests/license_files.rs` en el crate raíz de la lib) que verifique que son byte a byte iguales a los de la raíz. Documenta la decisión.
- `cargo publish --dry-run -p transbank-pos` sin warnings. Para `-sim` y `-cli` el dry-run falla porque `transbank-pos` no está en crates.io: verificar en su lugar con `cargo package -p <crate> --list` y reportarlo.

## 3. Reglas

- **Nunca** `cargo publish` sin `--dry-run`, nunca tags (AGENTS.md §4).

## 4. Tests

- Doctests de todos los ejemplos.
- Test de archivos de licencia idénticos (RF-04).

## 5. Definición de terminado

- [x] RF-01…RF-04.
- [x] AGENTS.md §8 en verde.
- [x] Salida de `cargo publish --dry-run`/`cargo package --list` en el Reporte.
- [x] Reporte completo.

## Reporte

Implementada por Claude (fallback: cuota Gemini en 0 %), 2026-10-21.

- **Rustdoc** (`transbank-pos`, página de crate): aviso unofficial y marca, tabla de capas, terminales soportados con el estado "sin validar", quick start real (`no_run`) y contra el simulador (doctest), tabla de operaciones (método → comando → respuesta), "Avoiding double charges" (ADR-04), features, MSRV y licencia. Ya había ejemplos en `PosIntegrado`, `SaleRequest`, `ResponseCode`, `MemoryTransport` y `SimTransport`. `RUSTDOCFLAGS=-D warnings` en verde; 29 doctests en la lib. `[package.metadata.docs.rs] all-features = true` en la lib y el simulador.
- **READMEs** de los tres crates en inglés: aviso, qué ofrece, instalación, ejemplo, features o uso, y enlaces.
- **README raíz** (cambio mínimo, archivo del orquestador): "Estado: en diseño" pasa a "implementado, sin validar con un POS real"; "Ejemplo (API objetivo)" pasa a "Ejemplo"; se precisa que el simulador es de POS Integrado.
- **`CHANGELOG.md`** (Keep a Changelog) con `[Unreleased]`.
- **Licencias por crate:** los symlinks de INFRA-01 pasaron a **copias idénticas**, como pide la tarea, porque en Windows un symlink de git puede quedar como un archivo de texto. `tests/license_files.rs` verifica que los 9 archivos (3 por crate) sean idénticos byte a byte a los de la raíz.
- **Metadatos:** se agrega `documentation` (docs.rs) en la lib y el simulador; el resto ya estaba (descripción "Unofficial…", licencia, repositorio, readme, keywords, categories, `rust-version`).
- **`include` de cada paquete:**
  - lib: `src`, `examples`, licencias, `NOTICE` y README; los ejemplos solo usan la lib.
  - simulador y CLI: `src`, licencias, `NOTICE` y README.
  - Los **tests quedan fuera** porque necesitan el simulador (dev-dependency por `path`, que se quita al publicar) y la raíz del workspace.
- **`cargo publish --dry-run -p transbank-pos --allow-dirty`:** "Packaged 31 files, 198.3KiB (49.5KiB compressed)", "Verifying… Finished", "aborting upload due to dry run".
  - **Desviación:** Cargo emite 7 avisos informativos `ignoring test <x> as tests/<x>.rs is not included in the published package`, consecuencia directa de excluir los tests.
  - Para eliminarlos habría que incluir en el paquete tests que no compilan fuera del workspace. Preferí avisos informativos a un `.crate` con tests rotos; no hay otros warnings.
- **`cargo package --list`:**
  - `transbank-pos-sim`: `Cargo.lock`, `Cargo.toml`, `LICENSE-APACHE`, `LICENSE-MIT`, `NOTICE`, `README.md`, `src/{lib,main,request,scenario,simulator,tcp,transport}.rs`.
  - `transbank-pos-cli`: `Cargo.lock`, `Cargo.toml`, licencias, `NOTICE`, `README.md`, `src/{format,json,main}.rs`.
  - No se pudo hacer el dry-run de los dos porque `transbank-pos` todavía no está en crates.io, como prevé la tarea.
- 211 tests en total en el workspace. AGENTS.md §8: los cinco comandos en verde; `validate.sh` OK. **No** se ejecutó `cargo publish` sin `--dry-run` ni se creó ningún tag.
