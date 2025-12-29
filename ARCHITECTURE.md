# WasmRust Architecture

This document provides a detailed overview of the WasmRust compiler architecture, repository structure, and technical contracts.

## 🏗️ WasmRust 5-Layer Architecture

WasmRust is structured as a **five-layer stack**, each independently useful and incrementally adoptable.

### **Layer 1: Core Language — Critical Additions**

#### 1️⃣ **Linear Types for WASM Resources**
WASM's [resource types](https://github.com/WebAssembly/component-model/blob/main/design/mvp/WIT.md#resources) (handles to DOM, files, GPU buffers) require **affine types** (use-once semantics):

```rust
// Linear type enforced at compile-time
#[wasm::linear]
struct CanvasContext(wasm::Handle);

impl CanvasContext {
    fn draw(&mut self) { /* ... */ }

    // Consuming method (moves ownership)
    fn into_bitmap(self) -> ImageData { /* ... */ }
}

// ❌ Compile error: can't use after move
let ctx = acquire_canvas();
let img = ctx.into_bitmap();
ctx.draw(); // ERROR: value moved
```

**Why**: Prevents resource leaks in Component Model integrations (e.g., Web APIs, WASI sockets).

#### 2️⃣ **Structured Concurrency (WASM Threads)**
Lacks **cancellation** and **scoped lifetimes**:

```rust
use wasm::thread::scope;

#[wasm::export]
fn parallel_transform(data: SharedSlice<f32>) -> Result<(), Error> {
    scope(|s| {
        for chunk in data.chunks(1000) {
            s.spawn(|| process(chunk)); // Lifetime tied to scope
        }
        // ← All threads joined here automatically
    })?;
    Ok(())
}
```

**Benefit**: Matches Trio/Kotlin coroutines patterns familiar to non-Rust devs.

#### 3️⃣ **Effect System for Side Effects**
Track I/O, JS calls, and atomics at type level:

```rust
// Pure functions (no side effects)
fn fibonacci(n: u32) -> u32 { /* ... */ }

// Effectful functions (explicit markers)
#[wasm::effect(js_call, atomic_read)]
fn fetch_and_cache(url: &str) -> Result<Vec<u8>, Error> {
    let data = js::fetch(url)?;  // js_call effect
    CACHE.store(url, data);       // atomic_write effect (inferred)
    Ok(data)
}
```

**Why**: Enables **tree-shaking dead effects** (e.g., remove all `js_call` code for server-side WASI builds).

---

### **Layer 2: Language Extensions — Component Model Deep Dive**

#### 🔌 **WIT Syntax as First-Class Citizen**
Should match [WIT IDL](https://component-model.bytecodealliance.org/design/wit.html) directly:

```rust
// Import definition (compiles to WIT)
#[wasm::wit]
interface crypto {
    use types.{bytes};

    resource key-pair {
        constructor(algorithm: string);
        sign: func(data: bytes) -> bytes;
    }

    hash-sha256: func(data: bytes) -> bytes;
}

// Usage (type-safe, no glue)
use crypto::{KeyPair, hash_sha256};

#[wasm::export]
fn sign_message(msg: &[u8]) -> Vec<u8> {
    let kp = KeyPair::new("ed25519");
    kp.sign(msg)
}
```

**Critical**: WIT → Rust codegen should be **bidirectional**:
- `wit-bindgen` generates Rust from WIT (existing)
- `rustc-wasm` generates WIT from Rust annotations (new)

#### 🧬 **Variance-Aware Generics**
WASM Component Model requires [**subtyping**](https://github.com/WebAssembly/component-model/blob/main/design/mvp/Subtyping.md):

```rust
// Covariant in return type
trait Serializer {
    type Output: wasm::Exportable;
    fn encode(&self) -> Self::Output;
}

// JSON serializer returns supertype (variant {json, error})
impl Serializer for JsonEncoder {
    type Output = variant { json(string), error(string) };
    // ...
}
```

**Why**: Allows **safe component substitution** (e.g., swap JSON encoder for MessagePack).

---

### **Layer 3: Runtime — Global Constraints**

#### 🌐 **Multi-Region Memory (China/EU Isolation)**
For GDPR/data residency:

```rust
#[wasm::memory(region = "eu-west-1", encryption = "AES256-GCM")]
static EU_DATA: wasm::Memory<8_000_000>; // 8 MB max

#[wasm::memory(region = "cn-north-1")]
static CN_DATA: wasm::Memory<8_000_000>;

#[wasm::export]
fn process_gdpr_data(user_id: &str) -> Result<(), Error> {
    let region = detect_region(user_id);
    match region {
        Region::EU => EU_DATA.write(user_id, data)?,
        Region::CN => CN_DATA.write(user_id, data)?,
        _ => return Err(Error::UnsupportedRegion),
    }
    Ok(())
}
```

**Implementation**: Compiler generates **multiple memory instances** per Component Model spec.

#### ⚡ **Streaming Compilation Hints**
Browser engines (V8, SpiderMonkey, JSC) compile WASM in parallel. Optimize layout:

```rust
#[wasm::compile_hints(
    tier = "baseline",  // Fast startup
    critical = ["render_frame", "handle_input"]
)]
mod ui {
    // Hot path functions
}

#[wasm::compile_hints(tier = "optimized")]
mod background_tasks {
    // Can take longer to compile
}
```

**Benefit**: 30-50% faster **Time to Interactive** on mobile devices.

---

### **Layer 4: Compiler — Architecture Shift**

#### 🔧 **Cranelift-First Backend**
LLVM is slow. Use [Cranelift](https://cranelift.dev/) (Rust-native, 10x faster compile times):

```
rustc-wasm architecture:
┌─────────────┐
│ Rust Source │
└──────┬──────┘
       │
       ▼
┌─────────────┐     ┌──────────────┐
│ HIR → MIR   │────▶│  Polonius    │ (borrow-checking)
└──────┬──────┘     └──────────────┘
       │
       ▼
┌─────────────┐
│ WasmIR      │ (WASM-specific intermediate form)
└──────┬──────┘
       │
       ├──▶ Cranelift ────▶ .wasm (fast, tier-1)
       │
       └──▶ LLVM ─────────▶ .wasm (optimized, tier-2)
```

**Strategy**:
- **Dev builds**: Cranelift only (~2s compile for 10k LOC)
- **Release builds**: LLVM + `wasm-opt` (~8s, 30% smaller)

#### 📊 **Profile-Guided Optimization (PGO) via Instrumentation**
Extend it:

```bash
# Step 1: Build with instrumentation
cargo wasm build --profile=instrumented

# Step 2: Collect profiles in production
wasm-runner ./app.wasm --collect-profile=prod.prof

# Step 3: Rebuild with profile data
cargo wasm build --release --pgo=prod.prof
```

**Data to collect**:
- Hot functions (inline aggressively)
- Cold imports (lazy-load via `WebAssembly.instantiateStreaming`)
- Memory access patterns (optimize data layout)

---

### **Layer 5: Tooling — Decentralized Ecosystem**

#### 🌐 **Component Registry — Avoid Centralization**
Don't replicate `crates.io` centralization. Use **federated registries**:

```bash
# Add multiple registries (prioritize non-US)
cargo wasm registry add bytecode-alliance https://registry.bytecodealliance.org
cargo wasm registry add wapm https://wapm.io
cargo wasm registry add apac https://wasm.asia/registry  # Asia-Pacific mirror
cargo wasm registry add self-hosted https://my-company.com/wasm

# Install with fallback
cargo wasm add crypto@1.2 --registry=apac,bytecode-alliance
```

**Why**: Resilience against geopolitical restrictions (e.g., npm blocking Russian IPs).

#### 🔍 **WASM-Aware Debugging**
Browser DevTools don't expose WASM memory well. Build **native tooling**:

```bash
# Attach debugger with memory inspection
wasm-gdb ./app.wasm --port 9229

# Visualize memory layout
(gdb) wasm mem visualize
Linear Memory [0x00000000 - 0x00100000]:
  0x00000000: [Stack] 16 KB
  0x00004000: [Heap]  64 KB (32 KB used)
  0x00014000: [Data]   4 KB (strings, constants)
```

---

## Repository Structure

```
wasmrust/
├── compiler/                # rustc extensions & backends
│   ├── codegen-cranelift/   # WASM-tuned Cranelift backend
│   └── codegen-llvm/        # WASM-optimized LLVM backend
│
├── crates/
│   ├── wasm/                # Core zero-cost WASM abstractions
│   └── wasm-macros/         # Proc macros for Component Model / WIT [planned]
│
├── tooling/
│   └── cargo-wasm/          # WASM-aware Cargo frontend [planned]
│
├── docs/
│   ├── SAFETY.md            # Unsafe invariants per type / crate
│   ├── compiler-contract.md # Formal compiler ↔ crate contracts
│   ├── RFCs/
│   └── architecture/
│
└── ReadMe.md
```
> Each crate has its own README and normative safety documentation (`SAFETY.md`) describing unsafe invariants and compiler contracts.

---

## Compiler ↔ Crate Contract

* The **compiler assumes** certain invariants when compiling code that uses `crates/wasm`:

  * `ExternRef<T>` and `FuncRef` are opaque handles with valid lifetime markers.
  * `SharedSlice<T>` contains only `Pod` types; aliasing and bounds are enforced.
  * Linear types (`#[wasm::linear]`) follow move semantics; the compiler assumes no implicit copies.
  * Component imports/exports use WIT-derived types; ABI must match exactly.
* Unsafe operations must maintain invariants documented in `SAFETY.md`.
* Compiler passes (e.g., verifier) will enforce these invariants at MIR and WasmIR level.
* Lints under `wasm-recognition` will detect misuses, such as:

  * `ExternRef` escaping a valid lifetime
  * Non-`Pod` types in `SharedSlice`
  * Invalid Component ABI usage

---

## Governance & Direction

*   Upstream-friendly design
*   Library APIs stabilize first, compiler features later
*   Avoids ecosystem fragmentation
*   RFC-driven feature evolution
