//! Component model support for WasmRust
//! 
//! This module provides basic abstractions for the WebAssembly Component Model.
//! This is a minimal implementation to support the core wasm crate functionality.

use crate::host::{get_host_capabilities};
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Simple signature representation
#[derive(Debug, Clone)]
pub struct Signature {
    pub name: String,
}

/// Component interface definition
#[derive(Debug, Clone)]
pub struct ComponentInterface {
    pub name: String,
    pub version: String,
}

impl ComponentInterface {
    pub fn new(name: String) -> Self {
        Self {
            name,
            version: "1.0.0".to_string(),
        }
    }
}

/// Component instance
#[derive(Debug)]
pub struct ComponentInstance {
    interface: ComponentInterface,
}

impl ComponentInstance {
    pub fn new(interface: ComponentInterface) -> Result<Self, ComponentError> {
        Ok(Self { interface })
    }
}

/// Component-related errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentError {
    /// Invalid component
    InvalidComponent,
    /// Component not found
    ComponentNotFound(String),
    /// Validation failed
    ValidationFailed(String),
}

impl core::fmt::Display for ComponentError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ComponentError::InvalidComponent => write!(f, "Invalid component"),
            ComponentError::ComponentNotFound(name) => write!(f, "Component not found: {}", name),
            ComponentError::ValidationFailed(msg) => write!(f, "Validation failed: {}", msg),
        }
    }
}

/// Initialize component model support
pub fn initialize_component_support() -> Result<(), ComponentError> {
    let caps = get_host_capabilities();
    if caps.component_model {
        // Component model is supported
        Ok(())
    } else {
        // Component model not supported, but we can continue
        Ok(())
    }
}