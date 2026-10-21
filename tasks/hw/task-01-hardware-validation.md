# Tarea HW-01: Validación con un POS real (manual, dueño)

| Atributo | Detalle |
|:--|:--|
| **ID** | `TASK-HW-01` |
| **Estado** | ⬜ pendiente |
| **Rama** | `dev/hw-01-traces` (solo trazas anonimizadas y el reporte) |
| **Depende de** | CLI-01 |
| **Restricciones** | `security-model.md` §6 · `docs/hardware-testing.md` |
| **Prioridad** | P0 para publicar |

## 1. Contexto

**No la ejecuta ningún agente.** El dueño conecta su POS Integrado y sigue `docs/hardware-testing.md` §2. El orquestador prepara la rama, ayuda a interpretar las trazas, cierra las preguntas abiertas de `docs/protocol-spec.md` §10 y crea las tareas `fix` necesarias.

## 2. Requerimientos

- RF-01 Los 20 pasos de `docs/hardware-testing.md` §2, con resultado anotado en el Reporte.
- RF-02 Trazas anonimizadas en `crates/transbank-pos/tests/fixtures/hw/` y un test de regresión por traza (lo escribe agy en una tarea `fix`/`test` posterior si hace falta).
- RF-03 Decisión de cada OPEN-0x (cerrada con evidencia o documentada como limitación).

## 3. Definición de terminado

- [ ] Ventas de prueba anuladas o cerradas el mismo día.
- [ ] `docs/protocol-spec.md` §10 actualizado por el orquestador.
- [ ] El dueño decide REL-01.

## Reporte

_(pendiente)_
