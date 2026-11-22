//! High-level clients (`PosIntegrado`, `PosAutoservicio`) built on top of the link.

mod autoservicio;
mod integrado;

pub use autoservicio::PosAutoservicio;
pub use integrado::PosIntegrado;
