# Requirements Document: WasmRust - Wasm-Native Rust Specialization

## Introduction

This document specifies the requirements for WasmRust, a Wasm-native Rust specialization that positions Rust as a first-class WebAssembly systems language. WasmRust competes directly with MoonBit on startup latency, binary size, and developer experience while preserving Rust's ecosystem advantage. The system achieves MoonBit-class performance through semantic-preserving compilation, streaming instantiation, zero-copy abstractions, and WasmGC readiness.

**Strategic Position**: WasmRust is not "better Rust → WASM" but "Rust as a Wasm-native systems language" that steals MoonBit's semantic timing, matches its startup and size performance, while exceeding it in ecosystem and control.

## Glossary

- **WasmRust**: Wasm-native Rust specialization for WebAssembly-first development
- **WasmIR**: Semantic intermediate representation preserving WASM-specific invariants
- **Streaming_Compilation**: Ordered compilation optimized for instantiation latency
- **Zero_Copy_Regions**: Typed memory regions eliminating marshaling overhead
- **Semantic_Contracts**: Compiler-crate agreements enabling aggressive optimization
- **ExternRef**: WebAssembly reference type for opaque host objects
- **SharedSlice**: Safe abstraction for shared memory access with Pod constraints
- **Pod**: Plain Old Data types safe for zero-copy operations
- **Component_Model**: WebAssembly Component Model specification for composable WASM modules
- **WIT**: WebAssembly Interface Types - the IDL for Component Model interfaces
- **Linear_Memory**: WebAssembly's contiguous memory space accessible to WASM modules
- **FuncRef**: WebAssembly reference type for function pointers
- **Cranelift**: Fast code generator used as alternative to LLVM
- **PGO**: Profile-Guided Optimization using runtime profiling data
- **WasmGC**: WebAssembly Garbage Collection proposal for native GC support
- **GcArray**: Garbage-collected array type for WasmGC environments
- **GcString**: Garbage-collected string type for WasmGC environments

## Requirements

### Requirement 1: Wasm-Native Performance Parity

**User Story:** As a web developer, I want WasmRust to match MoonBit's startup latency and binary size, so that I can achieve best-in-class performance without abandoning the Rust ecosystem.

#### Acceptance Criteria

1. WHEN compiling a "hello world" program with streaming profile, THE WasmRust_Compiler SHALL generate binaries under 2 KB (matching MoonBit performance)
2. WHEN instantiating WASM modules, THE WasmRust_Runtime SHALL achieve startup latency under 1ms for simple programs (matching MoonBit responsiveness)
3. WHEN using streaming compilation, THE WasmRust_Compiler SHALL emit ordered functions optimized for download and instantiation
4. THE WasmRust_Compiler SHALL provide thin monomorphization reducing code duplication by at least 40% compared to standard rustc
5. WHEN measuring against MoonBit benchmarks, THE WasmRust_System SHALL achieve equivalent or better performance on startup-critical metrics

### Requirement 2: Fast Compilation Performance

**User Story:** As a developer, I want fast compilation times during development, so that I can iterate quickly on WASM applications.

#### Acceptance Criteria

1. WHEN compiling 10,000 lines of code in development mode, THE WasmRust_Compiler SHALL complete within 2 seconds (improved from 5 seconds to match MoonBit performance)
2. WHEN using Cranelift backend, THE WasmRust_Compiler SHALL compile at least 5x faster than LLVM backend
3. WHEN incremental compilation is enabled, THE WasmRust_Compiler SHALL recompile only changed modules
4. THE WasmRust_Compiler SHALL provide separate development and release build profiles
5. WHEN using release mode, THE WasmRust_Compiler SHALL apply LLVM optimizations for maximum performance

### Requirement 3: Semantic-Preserving Compilation

**User Story:** As a systems programmer, I want WasmRust to preserve high-level semantics through compilation, so that the system can perform optimizations impossible with traditional LLVM lowering.

#### Acceptance Criteria

1. THE WasmIR_Representation SHALL explicitly encode externref, funcref, linear vs shared vs managed memory semantics
2. WHEN lowering from MIR to WasmIR, THE WasmRust_Compiler SHALL preserve ownership, linearity, and capability invariants
3. THE WasmIR_Specification SHALL serve as a stable semantic contract between frontend and backend optimizations
4. WHEN optimizing WasmIR, THE WasmRust_Compiler SHALL perform escape analysis, bounds-check elimination, and reference table elision
5. THE WasmRust_Compiler SHALL provide property tests validating MIR → WasmIR invariant preservation

### Requirement 4: Memory Safety and Type System

**User Story:** As a systems programmer, I want memory safety guarantees in WASM, so that I can write secure applications without runtime crashes.

#### Acceptance Criteria

1. THE WasmRust_Compiler SHALL enforce Rust's ownership and borrowing rules at compile time within the WasmRust dialect constraints
2. WHEN accessing shared memory, THE SharedSlice_Type SHALL prevent data races through type system constraints limited to Pod types
3. WHEN using linear types, THE WasmRust_Compiler SHALL enforce use-once semantics for WASM resources through compiler extensions
4. THE WasmRust_Compiler SHALL provide safe abstractions for ExternRef and FuncRef types with managed reference tables
5. WHEN capability annotations are used, THE WasmRust_Compiler SHALL track capabilities for optimization hints without type-level effect enforcement

### Requirement 5: Compiler-Crate Semantic Contracts

**User Story:** As a compiler engineer, I want formal contracts between the compiler and wasm crate, so that I can implement MoonBit-class optimizations while maintaining library-first evolution.

#### Acceptance Criteria

1. THE WasmRust_Compiler SHALL recognize and optimize whitelisted MIR patterns for ExternRef, SharedSlice, and Pod types
2. WHEN the wasm crate provides safety invariants, THE WasmRust_Compiler SHALL assume documented properties for optimization
3. THE WasmRust_Compiler SHALL implement wasmrust::recognition MIR patterns and wasmrust::semantic_contract lint group
4. WHEN optimizing recognized patterns, THE WasmRust_Compiler SHALL perform escape analysis on ExternRef and bounds-check elimination on SharedSlice<T: Pod>
5. THE WasmRust_System SHALL provide mechanical verification that all optimizations reference documented safety contracts

### Requirement 6: Zero-Copy Memory Abstractions

**User Story:** As a performance-conscious developer, I want zero-copy to be the default fast path, so that I can avoid marshaling overhead that MoonBit eliminates through native VM integration.

#### Acceptance Criteria

1. THE WasmRust_Compiler SHALL provide typed memory regions: Local<T>, SharedSlice<T: Pod>, and ExternRegion<T>
2. WHEN using Pod types, THE WasmRust_Compiler SHALL eliminate memcpy operations and enable direct memory access
3. THE WasmRust_Compiler SHALL reorder loads for Pod types and elide atomics when capability is absent
4. WHEN interfacing with JavaScript, THE WasmRust_Runtime SHALL provide zero-copy data transfer for supported types
5. THE WasmRust_System SHALL make zero-copy the default fast path, requiring explicit opt-in for marshaling

### Requirement 7: JavaScript Interoperability

**User Story:** As a web developer, I want efficient JavaScript integration with predictable performance characteristics, so that I can call JS APIs without complex bindings or unpredictable overhead.

#### Acceptance Criteria

1. WHEN calling JavaScript functions in supported host profiles, THE WasmRust_Runtime SHALL provide zero-copy data transfer and predictable boundary costs under 100 nanoseconds per call
2. WHEN passing Pod data between WASM and JS, THE WasmRust_Runtime SHALL avoid serialization through direct memory access
3. THE ExternRef_Type SHALL provide type-safe access to JavaScript objects with compile-time interface validation and runtime error handling
4. WHEN importing JS functions, THE WasmRust_Compiler SHALL generate direct WASM import declarations through managed reference tables
5. THE WasmRust_Runtime SHALL support bidirectional function calls with explicit ownership semantics and host-profile-specific error handling

### Requirement 8: Streaming Compilation Profile

**User Story:** As a web developer, I want perceptually instant startup times, so that my applications feel as responsive as MoonBit applications.

#### Acceptance Criteria

1. THE WasmRust_Compiler SHALL provide --profile=streaming with thin monomorphization and ordered function emission
2. WHEN using streaming profile, THE WasmRust_Compiler SHALL emit early export stubs and defer cold code
3. THE WasmRust_Toolchain SHALL default cargo-wasm to streaming profile with automatic instantiateStreaming hints
4. WHEN optimizing for startup, THE WasmRust_Compiler SHALL optimize section layout for download order and streaming instantiation
5. THE WasmRust_System SHALL achieve perceptually faster startup than standard Rust WASM, competing directly with MoonBit responsiveness

### Requirement 9: WasmGC Readiness Without Dependency

**User Story:** As a forward-looking developer, I want WasmRust to be ready for WasmGC without blocking on adoption, so that I can benefit from future GC capabilities while maintaining current compatibility.

#### Acceptance Criteria

1. THE WasmRust_Compiler SHALL design wasm crate types with dual lowerings: table index (today) and GC ref (future)
2. WHEN -Z wasm-gc-experimental flag is enabled, THE WasmRust_Compiler SHALL provide GC-aware lowering without bundled allocator
3. THE WasmRust_System SHALL maintain same API surface for both GC and non-GC lowerings
4. WHEN WasmGC is available, THE WasmRust_Runtime SHALL enable cycle collection with JavaScript integration
5. THE WasmRust_Compiler SHALL prepare for WasmGC as the long-term competitive moat against MoonBit

### Requirement 10: SIMD as First-Class Abstraction

**User Story:** As a performance engineer, I want native SIMD support that benefits from Wasm-native IR transforms, so that I can achieve the vectorization advantages that MoonBit gains from its IR design.

#### Acceptance Criteria

1. THE WasmRust_Compiler SHALL expose explicit SIMD types with portable fallbacks and capability-checked intrinsics
2. WHEN compiling SIMD operations, THE WasmRust_Compiler SHALL guarantee no scalarization unless required by target limitations
3. THE WasmRust_Compiler SHALL enable wasm-opt SIMD passes by default for maximum vectorization
4. WHEN SIMD capabilities are unavailable, THE WasmRust_Runtime SHALL provide efficient scalar fallbacks
5. THE WasmRust_System SHALL treat SIMD as a first-class abstraction comparable to MoonBit's native IR transforms

### Requirement 11: MoonBit-Class Tooling Performance

**User Story:** As a developer, I want iteration speed that matches MoonBit's development experience, so that Rust doesn't lose mindshare due to slower feedback loops.

#### Acceptance Criteria

1. THE WasmRust_Toolchain SHALL provide incremental WasmIR compilation with function-level recompilation
2. WHEN rebuilding projects, THE WasmRust_Compiler SHALL cache WasmIR artifacts and perform parallel semantic analysis
3. THE WasmRust_IDE_Integration SHALL provide fast feedback loops with partial program validity checking
4. WHEN developing iteratively, THE WasmRust_System SHALL match or exceed MoonBit's compilation and feedback speed
5. THE WasmRust_Toolchain SHALL provide compiler-driven diagnostics comparable to MoonBit's developer experience

### Requirement 12: Component Model Integration

**User Story:** As a system architect, I want seamless component composition that leverages WasmRust's zero-copy advantages, so that I can build modular systems without performance penalties.

#### Acceptance Criteria

1. THE WasmRust_Compiler SHALL generate Component Model compatible modules with zero-copy data sharing for Pod types
2. WHEN linking components, THE WasmRust_Runtime SHALL preserve zero-copy semantics across component boundaries
3. THE WasmRust_System SHALL provide type-safe component interfaces with automatic WIT generation
4. WHEN composing multi-language systems, THE WasmRust_Runtime SHALL maintain ABI safety and performance guarantees
5. THE WasmRust_Component_System SHALL exceed traditional WASM component performance through semantic preservation

### Requirement 13: Threading and Concurrency

**User Story:** As a performance-conscious developer, I want safe concurrent programming in WASM environments that support it, so that I can utilize multiple cores when available.

#### Acceptance Criteria

1. WHEN spawning threads in environments with SharedArrayBuffer support, THE WasmRust_Runtime SHALL provide structured concurrency with automatic cleanup
2. THE SharedSlice_Type SHALL enable safe shared memory access across WASM threads with compile-time data race prevention
3. WHEN using atomic operations, THE WasmRust_Compiler SHALL generate efficient WASM atomic instructions where supported
4. THE WasmRust_Runtime SHALL detect threading capability and provide fallback single-threaded execution when threads are unavailable
5. WHEN threads complete, THE WasmRust_Runtime SHALL automatically join all spawned threads within scoped lifetimes

### Requirement 14: Capability-Gated Execution

**User Story:** As a deployment engineer, I want explicit capability management that adapts to different WASM environments, so that I can deploy the same code across varying host capabilities.

#### Acceptance Criteria

1. THE WasmRust_Runtime SHALL require explicit opt-in for threading, shared memory, and SIMD capabilities
2. WHEN capabilities are unavailable, THE WasmRust_Runtime SHALL provide efficient fallback execution paths
3. THE WasmRust_Compiler SHALL perform capability detection at compile time and runtime for optimal code generation
4. WHEN targeting different environments, THE WasmRust_System SHALL adapt execution strategy based on available capabilities
5. THE WasmRust_Capability_System SHALL provide zero-overhead abstractions via IR layers, matching MoonBit's approach

### Requirement 15: Property-Based Validation Gates

**User Story:** As a systems engineer, I want provable invariants like MoonBit's confidence model, so that I can trust the system's correctness guarantees.

#### Acceptance Criteria

1. THE WasmRust_System SHALL enforce property tests for: no allocation for ExternRef ops, no memcpy for SharedSlice, ABI-safe component boundaries
2. WHEN building releases, THE WasmRust_CI SHALL require streaming-safe section layout and WasmIR invariant preservation as hard gates
3. THE WasmRust_Compiler SHALL provide mechanical verification of all semantic contract assumptions
4. WHEN optimizations are applied, THE WasmRust_System SHALL validate that they preserve documented invariants
5. THE WasmRust_Validation SHALL match MoonBit's confidence through property-based testing integrated into CI

### Requirement 16: Development Tooling

**User Story:** As a developer, I want comprehensive development tools, so that I can debug and optimize WASM applications effectively.

#### Acceptance Criteria

1. THE WasmRust_Toolchain SHALL provide a cargo-wasm command-line tool for project management
2. WHEN debugging applications, THE WasmRust_Debugger SHALL visualize linear memory layout and usage
3. THE WasmRust_Profiler SHALL collect runtime performance data for profile-guided optimization
4. WHEN analyzing binaries, THE WasmRust_Analyzer SHALL show size breakdowns by function and module
5. THE WasmRust_Toolchain SHALL integrate with existing Rust development environments

### Requirement 17: Multi-Language Component Support

**User Story:** As a system architect, I want to combine WasmRust with high-performance modules in other languages, so that I can optimize critical paths while maintaining safety.

#### Acceptance Criteria

1. WHEN importing Zig components in supported host profiles, THE WasmRust_Runtime SHALL provide type-safe bindings through Component Model with WIT interface validation
2. WHEN importing C components, THE WasmRust_Runtime SHALL handle memory ownership semantics correctly through explicit borrow-checking at component boundaries
3. THE WasmRust_Compiler SHALL validate component interfaces at compile time and reject incompatible ABI signatures
4. WHEN linking multi-language components, THE WasmRust_Runtime SHALL enable zero-copy data sharing for Pod types and frozen buffers only
5. THE WasmRust_Toolchain SHALL support hybrid project builds with multiple source languages through unified build orchestration

### Requirement 18: Global Registry and Distribution

**User Story:** As a developer in any region, I want reliable access to WASM components, so that I can build applications without geographic restrictions.

#### Acceptance Criteria

1. THE WasmRust_Registry SHALL support federated component registries across multiple regions
2. WHEN a primary registry is unavailable, THE WasmRust_Toolchain SHALL automatically fallback to mirror registries
3. THE WasmRust_Registry SHALL support self-hosted private registries for enterprise use
4. WHEN publishing components, THE WasmRust_Toolchain SHALL support multiple registry targets
5. THE WasmRust_Registry SHALL provide cryptographic verification of component integrity

### Requirement 19: Profile-Guided Optimization

**User Story:** As a performance engineer, I want to optimize WASM binaries based on production usage patterns, so that I can achieve maximum runtime performance.

#### Acceptance Criteria

1. WHEN building with instrumentation, THE WasmRust_Compiler SHALL embed profiling hooks in the generated WASM with deterministic profile collection and toolchain version tracking
2. WHEN collecting profiles, THE WasmRust_Runtime SHALL record function call frequencies and memory access patterns with provenance tracking and normalization
3. WHEN rebuilding with profile data, THE WasmRust_Compiler SHALL optimize hot paths and inline frequently called functions while maintaining reproducible builds with identical toolchain versions
4. THE WasmRust_Compiler SHALL support lazy loading of cold code paths based on profile data through Component Model dynamic linking
5. THE WasmRust_Profiler SHALL provide visualization of performance bottlenecks and optimization opportunities with actionable recommendations

### Requirement 20: Host Profile Compatibility

**User Story:** As a deployment engineer, I want WasmRust applications to work across different execution environments, so that I can deploy the same code to browsers, servers, and edge computing platforms.

#### Acceptance Criteria

1. THE WasmRust_Runtime SHALL detect host profile capabilities at load time and adapt execution accordingly
2. WHEN threading is unavailable, THE WasmRust_Runtime SHALL provide single-threaded fallback execution without code changes
3. WHEN Component Model is unsupported, THE WasmRust_Runtime SHALL provide polyfill implementations for basic component functionality according to the Component Model Support Matrix
4. THE WasmRust_Compiler SHALL generate host-profile-specific optimizations based on target environment declarations
5. WHEN memory region intents are unsupported, THE WasmRust_Runtime SHALL fail gracefully at load time with clear error messages

### Requirement 21: Compiler Architecture

**User Story:** As a Rust developer, I want to understand WasmRust's relationship to stable Rust, so that I can assess migration costs and compatibility.

#### Acceptance Criteria

1. THE WasmRust_Compiler SHALL be implemented as a rustc extension with custom codegen backend, where 80% of features are library-based, 15% use unstable compiler flags, and less than 5% require incompatible changes
2. WHEN compiling standard Rust code, THE WasmRust_Compiler SHALL produce functionally equivalent output to rustc with LLVM backend
3. THE WasmRust_Compiler SHALL document all deviations from Rust language specification in a compatibility matrix
4. WHEN linear types are used, THE WasmRust_Compiler SHALL provide clear migration path if upstream Rust adopts different syntax
5. THE WasmRust_Compiler SHALL maintain compatibility with the stable Rust ecosystem including crates.io dependencies

### Requirement 22: Ecosystem Preservation

**User Story:** As a Rust developer, I want to leverage existing Rust ecosystem while gaining Wasm-native performance, so that I don't sacrifice tooling and libraries for performance.

#### Acceptance Criteria

1. THE WasmRust_Compiler SHALL maintain compatibility with existing Rust crates and development tools
2. WHEN migrating from standard Rust, THE WasmRust_System SHALL provide clear migration paths and compatibility layers
3. THE WasmRust_Toolchain SHALL integrate with existing Rust development environments and workflows
4. WHEN using crates.io dependencies, THE WasmRust_Compiler SHALL optimize them using semantic contracts where applicable
5. THE WasmRust_Ecosystem SHALL exceed MoonBit in library availability while matching its performance characteristics

## Strategic Positioning Matrix

After implementing these requirements, WasmRust achieves the following competitive position:

| Dimension | Rust | MoonBit | WasmRust |
|-----------|------|---------|----------|
| Ecosystem | ✅ Huge | ❌ Small | ✅ Huge |
| Binary Size | ❌ | ✅ | ✅ |
| Startup Time | ❌ | ✅ | ✅ |
| WasmGC Ready | ❌ | ✅ | ✅ |
| Zero-Copy | ⚠️ | ✅ | ✅ |
| Streaming | ⚠️ | ✅ | ✅ |
| Control | ✅ | ⚠️ | ✅ |

## Build Profiles

### Streaming Profile (Default)
- Thin monomorphization for minimal size
- Ordered function emission for fast instantiation
- Early export stubs with cold code deferral
- Section layout optimized for download order
- Target: <2 KB binaries, <1ms startup

### Development Profile
- Fast incremental compilation with WasmIR caching
- Function-level recompilation
- Parallel semantic analysis
- Debug symbols and fast feedback loops
- Target: <100ms rebuild times

### Release Profile
- Full semantic optimization through WasmIR
- Aggressive zero-copy transformations
- SIMD vectorization and capability optimization
- Profile-guided optimization when available
- Target: Maximum runtime performance

## Non-Goals

- **Language Fork**: WasmRust remains Rust with WASM-specific optimizations
- **Universal Compatibility**: Breaking changes acceptable for significant performance gains
- **LLVM Replacement**: WasmIR augments rather than replaces LLVM backend
- **Manual Memory Management**: Rust's ownership system remains primary memory model
- **MoonBit API Compatibility**: Focus on performance parity, not API compatibility

## Success Metrics

### Performance Targets (MoonBit Parity)
- Binary size: <2 KB for hello world (vs MoonBit's ~2 KB)
- Startup latency: <1ms instantiation (vs MoonBit's ~1ms)
- Compilation speed: <100ms incremental builds (vs MoonBit's fast iteration)
- Zero-copy operations: 100% elimination of unnecessary marshaling
- SIMD utilization: Native vectorization without scalarization

### Ecosystem Advantages (vs MoonBit)
- Crate compatibility: 100% existing Rust crate ecosystem
- Tooling integration: Full Rust development environment support
- Migration cost: Minimal changes required from standard Rust
- Community size: Leverage existing Rust developer community
- Long-term sustainability: Built on proven Rust foundation

This requirements specification positions WasmRust as the "MoonBit path" for Rust developers, providing MoonBit-class performance while preserving Rust's ecosystem advantages.

## Strategic Positioning Matrix

After implementing these requirements, WasmRust achieves the following competitive position:

| Dimension | Rust | MoonBit | WasmRust |
|-----------|------|---------|----------|
| Ecosystem | ✅ Huge | ❌ Small | ✅ Huge |
| Binary Size | ❌ | ✅ | ✅ |
| Startup Time | ❌ | ✅ | ✅ |
| WasmGC Ready | ❌ | ✅ | ✅ |
| Zero-Copy | ⚠️ | ✅ | ✅ |
| Streaming | ⚠️ | ✅ | ✅ |
| Control | ✅ | ⚠️ | ✅ |

## Build Profiles

### Streaming Profile (Default)
- Thin monomorphization for minimal size
- Ordered function emission for fast instantiation
- Early export stubs with cold code deferral
- Section layout optimized for download order
- Target: <2 KB binaries, <1ms startup

### Development Profile
- Fast incremental compilation with WasmIR caching
- Function-level recompilation
- Parallel semantic analysis
- Debug symbols and fast feedback loops
- Target: <100ms rebuild times

### Release Profile
- Full semantic optimization through WasmIR
- Aggressive zero-copy transformations
- SIMD vectorization and capability optimization
- Profile-guided optimization when available
- Target: Maximum runtime performance

## Non-Goals

- **Language Fork**: WasmRust remains Rust with WASM-specific optimizations
- **Universal Compatibility**: Breaking changes acceptable for significant performance gains
- **LLVM Replacement**: WasmIR augments rather than replaces LLVM backend
- **Manual Memory Management**: Rust's ownership system remains primary memory model
- **MoonBit API Compatibility**: Focus on performance parity, not API compatibility

## Success Metrics

### Performance Targets (MoonBit Parity)
- Binary size: <2 KB for hello world (vs MoonBit's ~2 KB)
- Startup latency: <1ms instantiation (vs MoonBit's ~1ms)
- Compilation speed: <100ms incremental builds (vs MoonBit's fast iteration)
- Zero-copy operations: 100% elimination of unnecessary marshaling
- SIMD utilization: Native vectorization without scalarization

### Ecosystem Advantages (vs MoonBit)
- Crate compatibility: 100% existing Rust crate ecosystem
- Tooling integration: Full Rust development environment support
- Migration cost: Minimal changes required from standard Rust
- Community size: Leverage existing Rust developer community
- Long-term sustainability: Built on proven Rust foundation

## Host Profile Support

WasmRust explicitly supports the following execution environments:

### Browser Profile
- **Threading**: SharedArrayBuffer + COOP/COEP headers required
- **JS Interop**: Direct calls with managed reference tables
- **Component Model**: Partial support via polyfills
- **Memory Regions**: Not supported
- **WasmGC**: Native support in modern browsers
- **Performance Target**: <100ns JS call overhead

### Node.js Profile  
- **Threading**: Worker threads
- **JS Interop**: Native bindings
- **Component Model**: Via polyfill
- **Memory Regions**: Not supported
- **WasmGC**: Via V8 engine support
- **Performance Target**: <50ns JS call overhead

### Wasmtime Profile
- **Threading**: wasi-threads
- **JS Interop**: Host functions
- **Component Model**: Full native support
- **Memory Regions**: Configurable by host
- **WasmGC**: Native support
- **Performance Target**: <25ns host call overhead

### Embedded Profile
- **Threading**: Not supported
- **JS Interop**: Not supported  
- **Component Model**: Partial (static linking only)
- **Memory Regions**: Not supported
- **WasmGC**: Not supported (ownership mode only)
- **Performance Target**: Minimal runtime overhead

## Security and Trust Model

### Compiler Security
- THE WasmRust_Compiler SHALL validate all input sources and reject malicious code patterns
- THE WasmRust_Compiler SHALL provide cryptographic signatures for all generated artifacts
- THE WasmRust_Compiler SHALL maintain isolation between compilation units

### Runtime Security  
- THE WasmRust_Runtime SHALL enforce Component Model security boundaries
- THE WasmRust_Runtime SHALL validate all cross-component calls at runtime
- THE WasmRust_Runtime SHALL prevent unauthorized memory access between components

### Registry Security
- THE WasmRust_Registry SHALL require cryptographic signatures for all published components
- THE WasmRust_Registry SHALL provide audit trails for all component downloads and updates
- THE WasmRust_Registry SHALL support revocation of compromised components

## Appendix A: Baseline Definitions

### C Baseline for Size Comparison (Requirement 1.2)

**Compiler**: Clang 18.0 with wasm32-wasi target
**Flags**: `-Oz -flto=full -Wl,--gc-sections`
**Features**:
- Allocator: dlmalloc (same as wasm crate default)
- No exceptions (equivalent to panic=abort)
- No C++ stdlib (equivalent to no_std)

### Benchmark Applications

1. **Hello World**: Print "Hello, World!" to console, exit
2. **JSON Parser**: Parse 10KB JSON document using standard library
3. **Image Filter**: Apply 3x3 convolution to 512x512 RGBA image
4. **Crypto Hash**: SHA-256 over 1MB buffer

### Size Measurement

- **Tool**: wasm-opt --strip-debug --strip-producers
- **Metric**: Final .wasm file size after all optimizations
- **Threshold**: WasmRust output ≤ 3.0x C baseline for equivalent functionality

## Appendix B: Component Model Support Matrix

| Feature | Browser | Node.js | Wasmtime | Embedded |
|---------|---------|---------|----------|----------|
| Import/Export Functions | ✅ Native | ✅ Native | ✅ Native | ✅ Static |
| Resources (handles) | ⚠️ Polyfill | ⚠️ Polyfill | ✅ Native | ❌ |
| Canonical ABI | ✅ JS impl | ✅ Native | ✅ Native | ⚠️ Subset |
| Dynamic Linking | ❌ | ⚠️ Via loader | ✅ Native | ❌ |
| Futures/Streams | ❌ | ❌ | ✅ Preview 2 | ❌ |
| WasmGC Support | ✅ Native | ✅ Native | ✅ Native | ❌ |

**Legend**: 
- ✅ Full native support
- ⚠️ Partial/polyfill implementation  
- ❌ Not supported

### Polyfill Scope

**Browser/Node.js Polyfills Include**:
- Component imports/exports via JS wrapper functions
- Resource handles via WeakMap-based lifetime management
- Basic Canonical ABI for primitive types

**Browser/Node.js Polyfills Exclude**:
- Native resource destructors (manual cleanup required)
- Async/streaming interfaces (callback-based alternatives)
- Cross-component memory sharing (serialization required)

## Appendix C: Security Threat Model

### Threats Addressed

1. **Supply Chain Attack (High Priority)**
   - **Mitigation**: Cryptographic signatures on all registry components
   - **Detection**: Audit logs + transparency logs (like Certificate Transparency)

2. **Memory Safety Violations (Critical)**
   - **Mitigation**: Rust type system + linear types for WASM resources
   - **Detection**: Property-based testing with invalid memory access patterns

3. **Component Isolation Bypass (High Priority)**
   - **Mitigation**: Component Model security boundaries
   - **Detection**: Fuzz testing cross-component calls

### Threats NOT Addressed

1. **Side-Channel Attacks (e.g., Spectre)**
   - **Rationale**: Requires browser mitigations, not compiler-level

2. **Denial of Service via Resource Exhaustion**
   - **Rationale**: Host responsibility to set limits

3. **Timing Attacks on Cryptographic Code**
   - **Rationale**: Use constant-time crypto libraries (not compiler's job)

## Appendix D: Compiler-Crate Contract Specification

### Purpose

This contract defines the semantic boundary between the WasmRust compiler extension and the `wasm` crate to:
- Prevent unsound compiler assumptions
- Enable aggressive WASM-specific optimization safely
- Preserve library-first evolution
- Allow `wasm` crate to work on stable rustc
- Make upstreaming to rustc possible

### Fundamental Principles

1. **Zero-Cost Invariant**: All public types in `wasm` crate are `#[repr(transparent)]` or `#[repr(C)]`, layout-compatible with WASM counterparts, and free of hidden allocations
2. **No Semantic Magic**: The `wasm` crate provides no behavior that requires compiler support
3. **Escape Hatch Rule**: Everything the compiler assumes must be reproducible by a pure library implementation

### Type-Level Contracts

#### ExternRef<T>
```rust
#[repr(transparent)]
pub struct ExternRef<T> {
    handle: u32,
    _marker: PhantomData<T>,
}
```

**Compiler MAY assume**:
- Maps 1:1 to WASM externref
- Is opaque and non-dereferenceable
- Does not alias Rust memory
- Has no Rust-visible interior mutability

**Compiler MUST NOT assume**:
- Any lifetime or ownership beyond Rust typing
- GC behavior or host identity stability
- That equal handles represent equal objects

#### SharedSlice<'a, T: Pod>
```rust
pub struct SharedSlice<'a, T: Pod> {
    ptr: NonNull<T>,
    len: usize,
    _lifetime: PhantomData<&'a [T]>,
}
```

**Compiler MAY assume**:
- `T: Pod` implies no pointers, no drop glue, bitwise movable
- Backed by linear memory, safe for concurrent reads
- Writes governed by Rust aliasing rules

**Compiler MUST NOT assume**:
- Atomicity unless explicitly requested
- That threads exist (may lower to single-threaded)
- Memory is shared across components unless proven

#### Pod Trait
```rust
unsafe trait Pod: Copy + 'static {}
```

**Compiler MAY assume**:
- Trivially copyable with no invalid bit patterns
- Safe for zero-copy serialization

**Compiler MUST NOT assume**:
- Endianness normalization or stable ABI across targets
- That all `Copy` types are `Pod`

### MIR Pattern Matching Rules

The compiler is only allowed to recognize and optimize specific MIR patterns:

1. **ExternRef Pass-Through Pattern**: `_1 = ExternRef::new(_2); _3 = call foo(_1)`
2. **SharedSlice Load Pattern**: `_elt = (*(_slice.ptr + idx))` where `T: Pod`
3. **Pod Copy Pattern**: `_2 = _1` where `_1: T, T: Pod`
4. **Component Boundary Call Pattern**: `_0 = call component::import_X(_1, _2)`

### Optimization Safety Rules

**Allowed Optimizations**:
- Inline through wasm wrappers
- Merge monomorphizations when proven safe
- Remove unused exports
- Replace library calls with intrinsics

**Forbidden Optimizations**:
- Change observable behavior
- Introduce UB if wasm crate is replaced
- Assume unsafe blocks are safe
- Break Rust aliasing or lifetime rules

### Verification Requirements

All optimizations relying on this contract MUST:
- Reference the specific invariant section relied upon
- Be testable by removing the optimization and compiling with stable rustc
- Observe identical semantics in both cases

This requirements specification positions WasmRust as the "MoonBit path" for Rust developers, providing MoonBit-class performance while preserving Rust's ecosystem advantages.