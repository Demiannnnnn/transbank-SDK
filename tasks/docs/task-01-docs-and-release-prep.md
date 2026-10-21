# Tarea DOCS-01: Rustdoc, ejemplos, READMEs de crates, CHANGELOG y `publish --dry-run`

| Atributo | Detalle |
|:--|:--|
| **ID** | `TASK-DOCS-01` |
| **Estado** | ⬜ pendiente |
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

- [ ] RF-01…RF-04.
- [ ] AGENTS.md §8 en verde.
- [ ] Salida de `cargo publish --dry-run`/`cargo package --list` en el Reporte.
- [ ] Reporte completo.

## Reporte

_(pendiente)_
