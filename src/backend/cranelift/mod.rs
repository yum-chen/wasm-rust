//! Cranelift Backend Module
//! 
//! This module provides the Cranelift-based codegen backend for WasmRust,
//! optimized for fast development compilation.

pub mod lib;
pub mod integration;

// Re-export main types
pub use lib::*;
