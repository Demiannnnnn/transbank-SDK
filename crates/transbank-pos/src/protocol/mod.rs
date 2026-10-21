//! Sans-IO protocol core: control bytes, LRC, frame encoding and the frame decoder.
//!
//! Nothing in this module performs I/O or reads the clock, so it can be reused by
//! the link, the simulator and any async adapter.
