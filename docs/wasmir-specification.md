# WasmIR Specification

## Overview

WasmIR (WasmRust Intermediate Representation) is a stable intermediate representation between the rustc frontend and WasmRust backends. It serves as the boundary layer that encodes WASM-specific optimizations, ownership annotations, and capability hints.

## Design Goals

1. **Stability**: WasmIR provides a stable boundary between frontend and backends
2. **Optimization**: Enables WASM-specific optimizations not available in standard Rust MIR
3. **Safety**: Encodes ownership and type safety information for WebAssembly
4. **Capability**: Supports capability annotations for host profile optimization
5. **Performance**: Enables efficient code generation for WebAssembly targets

## Type System

### Core Types

#### Value Types
```
i32    // 32-bit signed integer
i64    // 64-bit signed integer  
f32    // 32-bit floating point
f64    // 64-bit floating point
```

#### Reference Types
```
externref<T>    // JavaScript object reference (type-safe)
funcref       // Function reference
```

#### Composite Types
```
array<T, N>     // Fixed-size array
struct<T> {     // Structure with fields
}
```

#### Linear Types
```
linear<T>       // Use-once semantics (consumed after use)
```

### Memory Model

#### Linear Memory
- WebAssembly linear memory with bounds checking
- Supports direct memory access through WASM instructions
- Optional bounds checking for development builds
- Zero-cost bounds removal for release builds

#### Shared Memory
- Thread-safe shared memory access
- Atomic operations for synchronization
- Support for SharedArrayBuffer in browsers

## Instruction Set

### Stack Operations
```
local.get N        // Get local variable N
local.set N V      // Set local variable N to value V
local.tee N V      // Get N and set N to V (return value)
```

### Constant Operations
```
i32.const V       // 32-bit integer constant
i64.const V       // 64-bit integer constant
f32.const V       // 32-bit float constant
f64.const V       // 64-bit float constant
```

### Binary Operations
```
i32.add L R       // Integer addition
i32.sub L R       // Integer subtraction
i32.mul L R       // Integer multiplication
i32.div_s L R     // Signed integer division
i32.rem_s L R     // Signed remainder
i32.and L R       // Bitwise AND
i32.or L R        // Bitwise OR
i32.xor L R       // Bitwise XOR
i32.shl L R       // Left shift
i32.shr_s L R     // Right shift (signed)
i32.lt_s L R       // Signed less than
i32.le_s L R       // Signed less than or equal
i32.gt_s L R       // Signed greater than
i32.ge_s L R       // Signed greater than or equal
```

### Unary Operations
```
i32.clz V         // Count leading zeros
i32.ctz V         // Count trailing zeros
i32.popcnt V      // Population count
i32.eqz V         // Equal to zero
```

### Memory Operations
```
i32.load N align=O offset=N    // Load with optional alignment
i32.store V align=O offset=N   // Store with optional alignment
i32.load8_u N offset=N          // Load 8-bit unsigned
i32.store8 V offset=N            // Store 8-bit
i32.load16_u N offset=N         // Load 16-bit unsigned
i32.store16 V offset=N            // Store 16-bit
```

### Control Flow
```
br target                    // Unconditional branch
br_if cond target else_target // Conditional branch
br_table V target0...targetN   // Branch table
return V                    // Return value
unreachable                  // Unreachable instruction
```

### Function Operations
```
call N args...             // Direct function call
call_indirect V args...      // Indirect function call
```

### Reference Type Operations
```
externref.new N              // Create new external reference
externref.get V field         // Get field from external reference
externref.set V field V       // Set field on external reference
funcref.new N               // Create function reference
```

### Component Model Operations
```
component.start                 // Start component instance
component.import N               // Import from component
component.export N               // Export to component
```

## Ownership Annotations

### Linear Types
```
linear.consume V           // Consume linear value
linear.move V              // Move linear value
linear.clone V             // Clone linear value (if supported)
linear.drop V             // Drop linear value
```

### Capabilities

### Threading
```
capability.threading         // Threading capability
capability.atomic           // Atomic operations
capability.shared_memory     // Shared memory access
```

### JavaScript Interop
```
capability.js_interop       // JavaScript interop
capability.externref         // External reference support
capability.funcref         // Function reference support
```

### Memory Regions
```
capability.memory_region "eu-west-1"    // Geographic memory region
capability.memory_encryption "AES256-GCM" // Memory encryption
```

## Optimization Hints

### Call Convention
```
call.indirect V signature=fast    // Fast calling convention
call.tail V                   // Tail call optimization
```

### Memory Access Patterns
```
memory.access_pattern streaming     // Streaming access
memory.access_pattern random       // Random access
memory.access_pattern sequential   // Sequential access
```

### Hot/Cold Separation
```
attribute.hot function_name          // Mark function as hot
attribute.cold function_name         // Mark function as cold
attribute.inline threshold=N         // Inline threshold
```

## Validation Rules

### Type Checking
1. All operations must be type-safe
2. Reference types must match their target types
3. Linear types must follow use-once semantics
4. Array bounds must be enforced where possible

### Control Flow
1. All branches must target valid basic blocks
2. Switch tables must be valid
3. Unreachable code must be properly marked

### Memory Safety
1. All memory accesses must be within bounds
2. Shared memory must use atomic operations
3. Linear memory must prevent use-after-free

### Component Model
1. All imports/exports must match interface definitions
2. Component boundaries must be validated
3. Type signatures must be compatible

## Implementation Guidelines

### Backend Mapping
1. Instructions map directly to WebAssembly opcodes
2. Types map to WebAssembly value types
3. Memory layout matches WebAssembly linear memory
4. Function signatures match WebAssembly calling convention

### Optimization Strategy
1. Instruction selection based on target capabilities
2. Register allocation optimized for WebAssembly
3. Code layout optimized for streaming
4. Dead code elimination for unused functions

### Error Handling
1. All validation errors must be precise and informative
2. Type errors must include expected vs actual types
3. Bounds errors must include safe alternatives
4. Capability errors must suggest alternatives

## Examples

### Simple Function
```rust
// WasmIR for: fn add(a: i32, b: i32) -> i32
let signature = Signature {
    params: vec![Type::I32, Type::I32],
    returns: Some(Type::I32),
};

let mut func = WasmIR::new("add", signature);
let local_a = func.add_local(Type::I32);
let local_b = func.add_local(Type::I32);
let local_result = func.add_local(Type::I32);

let instructions = vec![
    Instruction::LocalGet { index: local_a },
    Instruction::LocalGet { index: local_b },
    Instruction::BinaryOp {
        op: BinaryOp::Add,
        left: Operand::Local(local_result),
        right: Operand::Local(local_b),
    },
    Instruction::LocalSet { 
        index: local_result,
        value: Operand::Local(0), // Result of addition
    },
    Instruction::Return { 
        value: Some(Operand::Local(local_result))
    },
];

let terminator = Terminator::Return {
    value: Some(Operand::Local(local_result)),
};

func.add_basic_block(instructions, terminator);
```

### Memory Access Function
```rust
// WasmIR for: fn store_and_read(ptr: *mut i32, value: i32) -> i32
let signature = Signature {
    params: vec![Type::I32, Type::I32],  // ptr, value
    returns: Some(Type::I32),
};

let mut func = WasmIR::new("store_and_read", signature);
let local_ptr = func.add_local(Type::I32);
let local_value = func.add_local(Type::I32);
let local_loaded = func.add_local(Type::I32);

let instructions = vec![
    Instruction::LocalGet { index: local_ptr },
    Instruction::LocalGet { index: local_value },
    Instruction::MemoryStore {
        address: Operand::Local(local_ptr),
        value: Operand::Local(local_value),
        ty: Type::I32,
        align: Some(4),
        offset: 0,
    },
    Instruction::LocalGet { index: local_ptr },
    Instruction::MemoryLoad {
        address: Operand::Local(local_ptr),
        ty: Type::I32,
        align: Some(4),
        offset: 0,
    },
    Instruction::LocalSet {
        index: local_loaded,
        value: Operand::Local(1), // Loaded value
    },
    Instruction::Return {
        value: Some(Operand::Local(local_loaded)),
    },
];

let terminator = Terminator::Return {
    value: Some(Operand::Local(local_loaded)),
};

func.add_basic_block(instructions, terminator);
```

### Component Export
```rust
// WasmIR for: #[wasm::export] fn compute(x: i32) -> i32
let signature = Signature {
    params: vec![Type::I32],
    returns: Some(Type::I32),
};

let mut func = WasmIR::new("compute", signature);
func.add_capability(Capability::ComponentModel);

let local_x = func.add_local(Type::I32);
let local_result = func.add_local(Type::I32);

// Function that squares the input
let instructions = vec![
    Instruction::LocalGet { index: local_x },
    Instruction::BinaryOp {
        op: BinaryOp::Mul,
        left: Operand::Local(local_x),
        right: Operand::Local(local_x),
    },
    Instruction::LocalSet {
        index: local_result,
        value: Operand::Local(0),
    },
    Instruction::Return {
        value: Some(Operand::Local(local_result)),
    },
];

let terminator = Terminator::Return {
    value: Some(Operand::Local(local_result)),
};

func.add_basic_block(instructions, terminator);
```

## Migration Strategy

### From Rust MIR
1. Convert function signatures and types
2. Map basic blocks and control flow
3. Preserve debug information and source locations
4. Add capability annotations from attributes

### To WebAssembly
1. Direct instruction mapping where possible
2. Optimize for WebAssembly execution model
3. Apply WASM-specific optimizations
4. Generate efficient code layout

### Versioning
1. WasmIR version is tied to WasmRust compiler version
2. Backward compatibility guaranteed within major versions
3. Migration path provided for breaking changes
4. Tool support for automated migration

## Tooling Support

### Validation
- WasmIR validator for type checking
- Control flow analysis tools
- Memory safety verification
- Component model compliance checking

### Optimization
- Instruction selector for different targets
- Register allocator for WebAssembly
- Code layout optimizer
- Dead code eliminator

### Debugging
- Source location preservation
- Variable naming preservation
- Basic block visualization
- Instruction-level debugging

## Performance Considerations

### Code Size
- Prefer 32-bit operations where possible
- Use immediate values efficiently
- Optimize for binary size
- Eliminate unused code aggressively

### Execution Speed
- Optimize hot paths aggressively
- Use efficient instruction sequences
- Minimize memory traffic
- Exploit WebAssembly parallelism

### Memory Usage
- Stack allocation optimization
- Register pressure management
- Memory layout optimization
- Garbage collection avoidance

## Security Considerations

### Type Safety
- Strong type enforcement
- Memory bounds checking
- Reference type validation
- Component boundary enforcement

### Code Injection
- Instruction validation
- Control flow verification
- Memory access validation
- Component isolation

### Side Channels
- Speculative execution prevention
- Constant-time operations
- Memory access pattern randomization
- Component isolation enforcement
