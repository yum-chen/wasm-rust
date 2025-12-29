//! Cranelift Backend Module
//! 
//! This module provides the Cranelift-based codegen backend for WasmRust,
//! optimized for fast development compilation.

pub mod lib;
pub mod integration;
pub mod mir_lowering;
pub mod thin_monomorphization;

// Re-export main types
pub use lib::*;
pub use thin_monomorphization::*;
