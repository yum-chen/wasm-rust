# WasmRust Compiler-Crate Contract

**Version**: 1.0  
**Status**: Normative  
**Scope**: All compiler optimizations involving `wasm` crate types

---

## Purpose

This document defines the **formal contract** between the WasmRust compiler and the `wasm` crate. It specifies:

1. **MIR patterns** the compiler may recognize and optimize
2. **Invariants** the compiler may assume
3. **Verification requirements** for all optimizations
4. **Boundaries** of compiler trust

**Violation of this contract results in unsound optimizations.**

---

## Fundamental Principles

### 1. Zero-Cost Invariant
All public types are `#[repr(transparent)]` or `#[repr(C)]`, layout-compatible with WASM, free of hidden allocations.

### 2. No Semantic Magic
The `wasm` crate provides no behavior requiring compiler support beyond standard Rust semantics.

### 3. Escape Hatch Rule
Everything the compiler assumes must be reproducible by pure library implementation.

### 4. Mechanical Verification
All optimizations must reference documented invariants and pass MIR verification.

---

## Type Recognition (MIR Level)

### Canonical Signatures

| Type | MIR-Recognizable Shape | Layout Guarantee |
|------|------------------------|------------------|
| `ExternRef<T>` | `#[repr(transparent)] struct { u32, PhantomData<T> }` | `size_of() == 4` |
| `FuncRef<Args, Ret>` | `#[repr(transparent)] struct { u32, PhantomData<(Args, Ret)> }` | `size_of() == 4` |
| `SharedSlice<'a, T>` | `struct { NonNull<T>, usize, PhantomData<&'a [T]> }` | `size_of() == 2 * usize` |
| `Pod` | `unsafe trait Pod: Copy + Send + Sync + 'static + Sealed` | Bitwise movable |

### Recognition Rules

A type is considered a **Wasm Native Type** if:
- It is `#[repr(transparent)]` OR
- It lowers to a single scalar or scalar pair AND
- It implements the marker trait `WasmNative` (future)

---

## MIR Pattern Contracts

### Pattern 1: Zero-Cost Construction (Req 3.4)

**Invariant**: Construction, cloning, and access of Wasm native types must not allocate or branch.

**Allowed MIR Pattern**:
```mir
_0 = ExternRef { handle: _1, _phantom: const PhantomData }
```

**Forbidden MIR Patterns**:
```mir
❌ _2 = alloc::alloc(...)           // No allocation
❌ drop(_0)                         // No drop glue  
❌ switchInt(...)                   // No control flow
```

**MIR Assertion**:
```rust
assert!(
    !mir.contains(Alloc) && 
    !mir.contains(Drop) && 
    !mir.contains(SwitchInt),
    "ExternRef must be zero-cost"
);
```

**Validates**: Requirements 3.4, 4.1

---

### Pattern 2: Stable Host Identity (Req 4.2)

**Invariant**: `ExternRef<T>` cloning must preserve identity.

**Allowed MIR Pattern**:
```mir
_1 = copy _0  // Bitwise copy only
```

**Forbidden MIR Patterns**:
```mir
❌ _1 = transmute(_0)              // No handle transformation
❌ _1 = ExternRef::new(...)        // No reboxing
```

**MIR Assertion**:
```rust
assert!(
    externref_clone.lowering == BitwiseCopy,
    "ExternRef clone must be identity-preserving"
);
```

**Validates**: Requirements 4.2

---

### Pattern 3: Type Safety via Phantom Typing (Req 4.3)

**Invariant**: `ExternRef<T>` cannot be reinterpreted as `ExternRef<U>` without unsafe.

**Forbidden MIR Pattern**:
```mir
❌ _1 = _0 as ExternRef<U>         // Safe transmute forbidden
```

**Allowed (unsafe only)**:
```mir
✓ _1 = unsafe_transmute(_0)       // Explicit unsafe required
```

**MIR Assertion**:
```rust
if mir.contains(Transmute) && !context.is_unsafe() {
    error!("ExternRef type erasure requires unsafe");
}
```

**Validates**: Requirements 4.3

---

### Pattern 4: SharedSlice Alias Safety (Req 3.2, 6.1)

**Invariant**: `SharedSlice<T>` must never permit aliased mutable access.

**Forbidden MIR Pattern**:
```mir
❌ _1 = &mut (*_0)                // Two mutable borrows
❌ _2 = &mut (*_0)
```

**Allowed**:
```mir
✓ _1 = &(*_0)                     // Shared reads
✓ _2 = &(*_0)
✓ _1 = borrow_mut(_0, Capability::Exclusive)  // Capability-gated
```

**MIR Assertion**:
```rust
assert!(
    no_two_mut_borrows(sharedslice),
    "SharedSlice forbids aliased mutation"
);
```

**Validates**: Requirements 3.2, 6.1

---

### Pattern 5: Pod Enforcement (Req 3.2)

**Invariant**: Only `T: Pod` may appear in `SharedSlice<T>`.

**MIR Evidence**: At MIR, `T` is fully monomorphized.

**MIR Assertion**:
```rust
assert!(
    implements_trait(T, Pod) && is_sealed_impl(T, Pod),
    "SharedSlice<T> requires T: Pod with sealed implementation"
);
```

**Additional Verification**:
- Verify `T` has no interior mutability
- Verify `T` has no drop glue
- Verify `T` has no padding with semantic meaning

**Validates**: Requirements 3.2

---

### Pattern 6: Thread Capability Detection (Req 6.4)

**Invariant**: Threaded operations must be unreachable when threads are unsupported.

**Allowed MIR Pattern**:
```mir
if has_threads {
    sharedslice.atomic_load()
}
```

**Forbidden MIR Pattern**:
```mir
❌ atomic_load(...)               // Unconditional atomic ops
```

**MIR Assertion**:
```rust
if mir.contains(AtomicOp) && !mir.dominates(ThreadCapabilityCheck) {
    error!("Atomic op without thread capability guard");
}
```

**Validates**: Requirements 6.4

---

## Compiler Trust Boundaries

### Trusted Within Crate Boundary

The compiler **MAY assume** these properties **only within the `wasm` crate**:

1. **Layout Stability**: Types have stable layout across monomorphization
2. **No Hidden State**: No global state affects type behavior
3. **Sealed Implementations**: Pod trait cannot be implemented externally
4. **Capability Contracts**: Host capability checks are sound

### Untrusted Across Crate Boundaries

The compiler **MUST NOT assume**:

1. **External Pod Implementations**: Other crates cannot safely implement Pod
2. **ABI Stability**: Layout may change across crate versions
3. **Host Behavior**: Host environments may not honor contracts
4. **Memory Validity**: External pointers may be invalid

---

## Verification Infrastructure

### Required Compiler Components

1. **`wasm-recognition` Lint Group**:
   - `wasm_unverified_invariant_use`: Optimization lacks invariant reference
   - `wasm_illegal_mir_shape`: Transformation matches non-whitelisted pattern
   - `wasm_backend_specific_assumption`: Assumes LLVM/Cranelift-specific behavior
   - `wasm_unsafe_macro_semantics`: Assumes macro semantics beyond MIR

2. **`verify_wasm_invariants` MIR Pass**:
   - Runs after borrow checking, before backend lowering
   - Records invariant references and MIR patterns for each optimization
   - Enforces backend neutrality and negative pattern detection
   - Fails compilation with explicit diagnostics if violations found

### Verification Checklist

For each optimization, verify:

- [ ] References specific invariant from this document
- [ ] Matches only whitelisted MIR patterns
- [ ] Preserves semantics if `wasm` crate is replaced
- [ ] Does not assume undocumented layout properties
- [ ] Handles capability detection correctly

---

## Optimization Catalog

### Allowed Optimizations

1. **Inline through wasm wrappers** (if MIR pattern matches)
2. **Merge monomorphizations** when proven safe
3. **Remove unused exports** 
4. **Replace library calls with intrinsics** (with invariant reference)
5. **Reorder WASM sections** for performance
6. **Elide bounds checks** when statically proven safe
7. **Vectorize Pod array operations**

### Forbidden Optimizations

1. **Change observable behavior**
2. **Introduce UB if wasm crate is replaced**
3. **Assume unsafe blocks are safe**
4. **Break Rust aliasing or lifetime rules**
5. **Optimize based on undocumented layout**
6. **Skip capability checks**
7. **Assume external Pod implementations are sound**

---

## Future Evolution

### Stability Guarantees

1. **MIR patterns** in this document are stable across compiler versions
2. **Invariant references** will be maintained for backward compatibility
3. **New patterns** will be added via formal RFC process
4. **Breaking changes** require major version bump

### Extension Points

1. **New Wasm Native Types** can be added following the recognition rules
2. **Additional MIR patterns** can be whitelisted via compiler updates
3. **Enhanced verification** can be added without breaking existing code
4. **Host profile expansion** can add new capability patterns

---

## Compliance Testing

### Required Test Coverage

1. **MIR pattern recognition** tests for each whitelisted pattern
2. **Negative tests** for forbidden patterns
3. **Cross-compilation** tests for layout stability
4. **Capability detection** tests for all host profiles
5. **Property-based tests** for invariant violations

### Continuous Verification

1. **CI integration** runs verification on every commit
2. **Fuzzing** tests adversarial MIR patterns
3. **Cross-platform** testing ensures portability
4. **Performance regression** testing validates optimizations

---

## Summary

This contract enables:

- **Safe compiler optimizations** through mechanical verification
- **Predictable performance** through documented patterns
- **Forward compatibility** through stable interfaces
- **Ecosystem trust** through formal guarantees

**Violation of any aspect of this contract constitutes a compiler bug.**

For questions or proposed changes, file an issue with the WasmRust compiler team.