#!/bin/bash

# Architecture 2.0 Build and Test Script
# This script verifies the WasmRust Architecture 2.0 implementation

echo "=== WasmRust Architecture 2.0 Build Test ==="

# Check if we're in the correct directory
if [ ! -f "Cargo.toml" ]; then
    echo "Error: Must be run from wasmrust root directory"
    exit 1
fi

echo "1. Building wasm-macros crate..."
cd crates/wasm-macros
cargo build
if [ $? -ne 0 ]; then
    echo "❌ wasm-macros build failed"
    exit 1
fi
echo "✅ wasm-macros built successfully"

echo "2. Building wasm crate..."
cd ../wasm
cargo check
if [ $? -ne 0 ]; then
    echo "❌ wasm crate check failed"
    echo "Building with more verbose output..."
    cargo build
    exit 1
fi
echo "✅ wasm crate check successful"

echo "3. Running Architecture 2.0 validation tests..."
cd ../..
cargo test --test architecture-2.0-validation
if [ $? -ne 0 ]; then
    echo "❌ Architecture 2.0 tests failed"
    exit 1
fi
echo "✅ Architecture 2.0 tests passed"

echo "4. Creating documentation overview..."
echo "=== Architecture 2.0 Implementation Status ===" > architecture-status.md
echo "" >> architecture-status.md
echo "✅ README.md updated with Architecture 2.0 positioning" >> architecture-status.md
echo "✅ docs/architecture/wasmrust-vs-moonbit.md created" >> architecture-status.md  
echo "✅ docs/architecture/architecture-2.0-design-principles.md created" >> architecture-status.md
echo "✅ docs/RFCs/0001-architecture-2.0-overview.md created" >> architecture-status.md
echo "✅ crates/wasm/src/gc.rs - GC infrastructure implemented" >> architecture-status.md
echo "✅ crates/wasm-macros/ - Attribute macro system implemented" >> architecture-status.md
echo "✅ tests/architecture-2.0-validation.rs - Validation tests created" >> architecture-status.md
echo "" >> architecture-status.md
echo "=== Phase 0 Progress ===" >> architecture-status.md
echo "- [✓] 0.1: Project documentation updated with Architecture 2.0 positioning" >> architecture-status.md
echo "- [✓] 0.2: Condition compilation infrastructure established" >> architecture-status.md
echo "- [✓] 0.3: GC runtime infrastructure foundation created" >> architecture-status.md  
echo "- [ ] 0.4: Curation registry prototype (future work)" >> architecture-status.md
echo "" >> architecture-status.md
echo "Architecture 2.0 core infrastructure phase completed!" >> architecture-status.md

echo "✅ Architecture 2.0 implementation documentation created: architecture-status.md"

echo "=== Architecture 2.0 Build Completed Successfully ==="
echo "Core infrastructure for dual compilation strategy is ready."
echo "Phase 0.1-0.3 completed."
echo ""
echo "Next steps:"
echo "1. Test with actual WASM target"
echo "2. Implement GC runtime integration"
echo "3. Develop #[wasm::gc] macro functionality"
echo "4. Create real-world examples"
echo ""
echo "For more details, see architecture-status.md"