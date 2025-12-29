# WasmRust Curation Registry

A federated registry system designed specifically for WASM-compatible Rust crates. This registry provides automated testing, compatibility validation, and curation features to ensure high-quality WASM support for Rust crates.

## 🚀 Overview

The WasmRust Curation Registry is a key component of the Architecture 2.0 initiative, enabling:

- **Automated WASM compatibility testing**
- **Quality assurance for dual compilation strategy**
- **Federated registry architecture**
- **Community-driven curation**

## 📋 Features

### Core Features
- **WASM Compatibility Testing** - Automated testing pipeline for compiling and running Rust crates in WebAssembly
- **GC Readiness Validation** - Checks for garbage collection compatibility in WASM targets
- **Dual Compilation Support** - Validates that crates work in both native and WASM environments
- **Federated Architecture** - Multiple registries can interoperate and sync
- **Digital Signatures** - Secure crate metadata with cryptographic signatures

### Quality Assurance
- **Automated Testing Pipeline** - Comprehensive test suite for WASM compatibility
- **Performance Benchmarking** - Compare native vs WASM performance
- **Memory Usage Analysis** - Track resource consumption patterns
- **Dependency Analysis** - Map compatibility across dependency trees

### Developer Experience
- **RESTful API** - Programmatic access to registry services
- **CLI Tool** - Command-line interface for developers
- **CI/CD Integration** - GitHub Actions and GitLab CI templates
- **Web Dashboard** - Browser-based interface (planned)

## 🏗️ Architecture

### Components
1. **API Server** - RESTful HTTP API for crate management
2. **Database Layer** - SQLite/PostgreSQL for metadata storage
3. **Testing Pipeline** - Automated compilation and execution tests
4. **CLI Tool** - Developer command-line interface
5. **Sync Service** - Registry-to-registry synchronization

### Data Model
```rust
struct CrateMetadata {
    id: String,
    name: String,
    version: String,
    wasm_compatibility: WasmCompatibility,
    gc_ready: bool,
    dual_compilation: bool,
    test_results: Vec<TestResult>,
    // ... additional fields
}
```

## 🚀 Getting Started

### Prerequisites
- Rust 1.70+
- SQLite (or PostgreSQL)
- WebAssembly target: `rustup target add wasm32-unknown-unknown`
- wasmtime or other WASM runtime

### Installation

1. **Clone the repository**
```bash
git clone https://github.com/wasmrust/wasm-crates-registry
cd wasm-crates-registry
```

2. **Build the project**
```bash
cargo build --release
```

3. **Configure environment**
```bash
export DATABASE_URL="sqlite:registry.db"
export HOST="127.0.0.1"
export PORT="8080"
```

4. **Run the server**
```bash
cargo run --bin wasm-crates-registry
```

### Using the CLI

1. **Check registry health**
```bash
cargo run --bin wasm-crates-registry-cli -- health
```

2. **List crates**
```bash
cargo run --bin wasm-crates-registry-cli -- list
```

3. **Submit a crate**
```bash
cargo run --bin wasm-crates-registry-cli -- submit --manifest path/to/Cargo.toml
```

4. **Run tests for a crate**
```bash
cargo run --bin wasm-crates-registry-cli -- test <crate-id>
```

## 🔧 Configuration

### Environment Variables
- `DATABASE_URL` - Database connection string
- `HOST` - Server host address
- `PORT` - Server port
- `REGISTRY_NAME` - Registry instance name
- `PUBLIC_KEY` - Signing public key

### Registry Configuration
The registry can be configured via the API or CLI:

```bash
# Get current configuration
cargo run --bin wasm-crates-registry-cli -- config

# Update configuration (requires API integration)
curl -X PUT http://localhost:8080/registry -d @config.json
```

## 📊 API Reference

### Crate Management
- `GET /crates` - List crates with filtering
- `POST /crates` - Submit new crate
- `GET /crates/{id}` - Get crate details
- `PUT /crates/{id}` - Update crate
- `DELETE /crates/{id}` - Delete crate

### Testing
- `POST /tests/{id}` - Run tests for crate
- `GET /tests/{id}` - Get test results

### Registry Management
- `GET /registry` - Get configuration
- `PUT /registry` - Update configuration
- `POST /registry/sync` - Sync with peer registries

## 🧪 Testing Pipeline

The automated testing pipeline includes:

### Compilation Tests
- Native compilation (x86_64, aarch64)
- WASM compilation (wasm32-unknown-unknown)
- Cross-compilation validation

### Functionality Tests
- Unit test execution
- Integration tests
- WASM-specific functionality

### Performance Tests
- Execution time benchmarks
- Memory usage analysis
- Binary size comparison

## 🔒 Security

### Digital Signatures
- All crate metadata is cryptographically signed
- Registry instances verify signatures
- Developer keys are registered and verified

### Access Control
- API key authentication
- Rate limiting
- Role-based permissions

### Data Integrity
- Immutable audit logs
- Hash-based content verification
- Tamper-evident records

## 🌐 Federation

### Registry Sync
- Peer-to-peer synchronization
- Conflict resolution strategies
- Delta-based updates
- Secure communication

### Quality Gates
- Minimum compatibility requirements
- Performance thresholds
- Community review process
- Automated quality scoring

## 🚧 Development

### Project Structure
```
src/
├── api.rs          # RESTful API implementation
├── database.rs     # Database layer
├── schema.rs       # Data structures
├── testing/        # Testing pipeline
└── cli/            # Command-line interface
```

### Running Tests
```bash
# Unit tests
cargo test

# Integration tests
cargo test --test integration

# Specific test categories
cargo test --test compilation
cargo test --test performance
```

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### Areas for Contribution
- Testing pipeline improvements
- WASM runtime integrations
- Performance optimizations
- Additional WASM targets
- Documentation

## 📄 License

This project is licensed under the MIT OR Apache-2.0 licenses.

## 🙏 Acknowledgments

- Rust WebAssembly Working Group
- wasmtime and wasmer teams
- crates.io team for inspiration
- WebAssembly community

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/wasmrust/wasm-crates-registry/issues)
- **Discussions**: [GitHub Discussions](https://github.com/wasmrust/wasm-crates-registry/discussions)
- **Documentation**: [GitHub Wiki](https://github.com/wasmrust/wasm-crates-registry/wiki)

---

**Part of the WasmRust Architecture 2.0 Initiative** - *Native Rust for systems. GC-ready Rust for WebAssembly.*