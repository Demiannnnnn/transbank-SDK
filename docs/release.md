# Publicación en crates.io (REL-01)

**Solo con autorización explícita del dueño** y después de HW-01 (decisión del dueño 2026-10-03: "no publicar sin validar con un POS real en modo integrado"). Ningún agente ni el orquestador ejecuta `cargo publish` sin `--dry-run`, ni crea tags.

## 1. Requisitos previos

- [ ] HW-01 completo; preguntas abiertas de `protocol-spec.md` §10 cerradas o documentadas como limitación.
- [ ] `develop-AGENTS` integrado por el dueño en `develop` y luego en `main`.
- [ ] Definición de terminado (AGENTS.md §8) en verde en Linux, macOS y Windows (CI manual).
- [ ] `CHANGELOG.md` con la versión y la fecha.
- [ ] Metadatos en cada `Cargo.toml`: `description` (empieza por "Unofficial"), `license = "MIT OR Apache-2.0"`, `repository`, `documentation`, `readme`, `keywords` (`transbank`, `pos`, `chile`, `serial`, `payments`), `categories` (`hardware-support`, `finance`, `command-line-utilities` para la CLI), `rust-version`, `include` con `NOTICE` y las licencias (LEG-01).
- [ ] Nombres libres en crates.io (verificados 2026-10-21: `transbank-pos`, `transbank-pos-sim`, `transbank-pos-cli` → 404).
- [ ] `cargo publish --dry-run -p <crate>` sin advertencias para los tres.
- [ ] README de cada crate en inglés con el aviso "unofficial" (LEG-03).

## 2. Orden

1. `transbank-pos` (sin dependencias internas).
2. `transbank-pos-sim` (depende de `transbank-pos` con versión, no solo `path`).
3. `transbank-pos-cli`.

## 3. Pasos (los ejecuta el dueño)

```bash
git switch main && git pull
cargo publish -p transbank-pos
cargo publish -p transbank-pos-sim
cargo publish -p transbank-pos-cli
git tag v0.1.0 && git push origin v0.1.0
```

Luego: release en GitHub con el CHANGELOG, y en `rework-sistema-facturacion` reemplazar cualquier dependencia `path`/`git` por la versión publicada.
