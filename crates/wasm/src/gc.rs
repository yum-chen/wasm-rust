//! GC (Garbage Collection) infrastructure for WasmRust Architecture 2.0
//! 
//! This module provides conditional GC support through attribute-driven
//! compilation. Types marked with `#[wasm::gc]` will use GC semantics
//! when targeting WebAssembly, while maintaining standard Rust semantics
//! for native targets.

use core::marker::PhantomData;
use core::ptr;
use alloc::vec::Vec;
use alloc::string::String;

/// Marker trait for types that can be GC-managed in WebAssembly targets
/// 
/// This trait is automatically derived for types marked with `#[wasm::gc]`
/// and provides the compiler with semantic information for GC optimization.
pub unsafe trait GcManaged: 'static {
    /// Marks this object and all reachable objects during garbage collection
    fn gc_mark(&self);
    
    /// Traces all references from this object during GC
    fn gc_trace(&self);
    
    /// Returns the size of this object for memory accounting
    fn gc_size(&self) -> usize;
}

impl GcManaged for u8 {
    fn gc_mark(&self) {}
    fn gc_trace(&self) {}
    fn gc_size(&self) -> usize { core::mem::size_of::<u8>() }
}

impl GcManaged for u32 {
    fn gc_mark(&self) {}
    fn gc_trace(&self) {}
    fn gc_size(&self) -> usize { core::mem::size_of::<u32>() }
}

impl GcManaged for i32 {
    fn gc_mark(&self) {}
    fn gc_trace(&self) {}
    fn gc_size(&self) -> usize { core::mem::size_of::<i32>() }
}

impl<T: GcManaged> GcManaged for Vec<T> {
    fn gc_mark(&self) {
        for item in self {
            item.gc_mark();
        }
    }
    fn gc_trace(&self) {
        for item in self {
            item.gc_trace();
        }
    }
    fn gc_size(&self) -> usize {
        self.len() * core::mem::size_of::<T>()
    }
}

/// GC-managed string type for WebAssembly targets
/// 
/// In WebAssembly targets, this type uses garbage collection for memory management.
/// In native targets, it behaves identically to `std::string::String`.
#[cfg_attr(target_family = "wasm", repr(transparent))]
pub struct GcString {
    #[cfg(target_family = "wasm")]
    inner: *mut u8,  // GC-managed string pointer in WASM
    
    #[cfg(not(target_family = "wasm"))]
    inner: String,  // Standard string in native
}

impl GcString {
    /// Creates a new GC-managed string
    pub fn new() -> Self {
        #[cfg(target_family = "wasm")]
        {
            Self {
                inner: ptr::null_mut(),
            }
        }
        
        #[cfg(not(target_family = "wasm"))]
        {
            Self {
                inner: String::new(),
            }
        }
    }
    
    /// Creates a string from a string slice
    pub fn from_str(s: &str) -> Self {
        #[cfg(target_family = "wasm")]
        {
            // Placeholder - would allocate in GC memory
            Self {
                inner: ptr::null_mut(),
            }
        }
        
        #[cfg(not(target_family = "wasm"))]
        {
            Self {
                inner: String::from(s),
            }
        }
    }
    
    /// Gets the length of the string
    pub fn len(&self) -> usize {
        #[cfg(target_family = "wasm")]
        {
            unsafe { gc_string_len(self.inner) }
        }
        
        #[cfg(not(target_family = "wasm"))]
        {
            self.inner.len()
        }
    }
    
    /// Checks if the string is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

unsafe impl GcManaged for GcString {
    fn gc_mark(&self) {
        #[cfg(target_family = "wasm")]
        {
            // In WASM, mark this string for GC
            unsafe { gc_mark_string(self.inner) };
        }
        
        #[cfg(not(target_family = "wasm"))]
        {
            // In native, no GC action needed
            let _ = self;
        }
    }
    
    fn gc_trace(&self) {
        // Strings don't contain references to trace
    }
    
    fn gc_size(&self) -> usize {
        #[cfg(target_family = "wasm")]
        {
            unsafe { gc_string_len(self.inner) }
        }
        
        #[cfg(not(target_family = "wasm"))]
        {
            self.inner.len()
        }
    }
}

/// GC-managed array type for WebAssembly targets
#[cfg_attr(target_family = "wasm", repr(transparent))]
pub struct GcArray<T> {
    #[cfg(target_family = "wasm")]
    elements: *mut T,  // GC-managed array pointer
    
    #[cfg(not(target_family = "wasm"))]
    elements: Vec<T>,  // Standard vector in native
    
    len: usize,
    _marker: PhantomData<T>,
}

impl<T> GcArray<T> {
    /// Creates a new GC-managed array
    pub fn new() -> Self {
        #[cfg(target_family = "wasm")]
        {
            Self {
                elements: core::ptr::null_mut(),
                len: 0,
                _marker: PhantomData,
            }
        }
        
        #[cfg(not(target_family = "wasm"))]
        {
            Self {
                elements: Vec::new(),
                len: 0,
                _marker: PhantomData,
            }
        }
    }
    
    /// Pushes an element to the array
    pub fn push(&mut self, value: T) {
        #[cfg(target_family = "wasm")]
        {
            unsafe { gc_array_push(self.elements, value) };
            self.len += 1;
        }
        
        #[cfg(not(target_family = "wasm"))]
        {
            self.elements.push(value);
            self.len = self.elements.len();
        }
    }
    
    /// Gets the length of the array
    pub fn len(&self) -> usize {
        self.len
    }
}

unsafe impl<T: GcManaged> GcManaged for GcArray<T> {
    fn gc_mark(&self) {
        #[cfg(target_family = "wasm")]
        {
            unsafe { gc_mark_array(self.elements as *mut u8) };
        }
        
        #[cfg(not(target_family = "wasm"))]
        {
            for item in &self.elements {
                item.gc_mark();
            }
        }
    }
    
    fn gc_trace(&self) {
        #[cfg(target_family = "wasm")]
        {
            // In WASM, trace is handled by GC system
        }
        
        #[cfg(not(target_family = "wasm"))]
        {
            for item in &self.elements {
                item.gc_trace();
            }
        }
    }
    
    fn gc_size(&self) -> usize {
        self.len * core::mem::size_of::<T>()
    }
}

impl<T> GcArray<T> {
    /// Gets an element by index without bounds checking
    pub unsafe fn get_unchecked(&self, index: usize) -> &T {
        #[cfg(target_family = "wasm")]
        {
            &*self.elements.add(index)
        }
        
        #[cfg(not(target_family = "wasm"))]
        {
            &self.elements[index]
        }
    }
}

/// GC-managed box type for WebAssembly targets
#[cfg_attr(target_family = "wasm", repr(transparent))]
pub struct GcBox<T: ?Sized> {
    #[cfg(target_family = "wasm")]
    ptr: *mut T,  // GC-managed pointer
    
    #[cfg(not(target_family = "wasm"))]
    ptr: alloc::boxed::Box<T>,  // Standard box in native
}

impl<T> GcBox<T> {
    /// Creates a new GC-managed box
    pub fn new(value: T) -> Self {
        #[cfg(target_family = "wasm")]
        {
            let ptr = unsafe { gc_allocate::<T>() };
            unsafe { ptr::write(ptr, value) };
            Self { ptr }
        }
        
        #[cfg(not(target_family = "wasm"))]
        {
            Self { ptr: alloc::boxed::Box::new(value) }
        }
    }
}

unsafe impl<T: GcManaged + ?Sized> GcManaged for GcBox<T> {
    fn gc_mark(&self) {
        #[cfg(target_family = "wasm")]
        {
            unsafe { gc_mark_ptr(self.ptr) };
        }
    }
    
    fn gc_trace(&self) {
        self.as_ref().gc_trace();
    }
    
    fn gc_size(&self) -> usize {
        core::mem::size_of::<T>()
    }
}

// External GC functions for WebAssembly target
#[cfg(target_family = "wasm")]
extern "C" {
    fn gc_mark_string(ptr: *mut u8);
    fn gc_mark_array(ptr: *mut u8);
    fn gc_mark_ptr(ptr: *mut u8);
    fn gc_string_len(ptr: *mut u8) -> usize;
    fn gc_array_push<T>(array: *mut T, value: T);
    fn gc_allocate<T>() -> *mut T;
}

/// Attribute macro for marking types as GC-managed in WebAssembly targets
/// 
/// This macro enables conditional GC semantics without breaking native compatibility.
pub use wasm_macros::gc;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gc_string_creation() {
        // Should compile and work in both targets
        let _string = GcString::new();
    }

    #[test]
    fn test_gc_array_operations() {
        let mut array = GcArray::<u32>::new();
        array.push(42);
        assert_eq!(array.len(), 1);
    }
}