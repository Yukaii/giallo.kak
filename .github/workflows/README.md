# GitHub Actions CI/CD

This repository uses GitHub Actions for automated builds and releases.

## Workflow Overview

The `.github/workflows/build-release.yml` workflow handles:

1. **Cloning giallo dependency** - Clones the upstream giallo repository
2. **Generating builtin dump** - Runs Node.js scripts to extract grammar metadata and generates `builtin.msgpack`
3. **Building giallo-kak** - Compiles the binary for multiple platforms
4. **Running tests** - Executes test suite
5. **Creating releases** - Publishes binaries when tags are pushed

## Build Process

### Prerequisites

- **Rust toolchain** (stable)
- **Node.js** (v20+)
- **npm**

### Build Steps

```bash
# 1. Clone giallo repository
git clone https://github.com/getzola/giallo.git

# 2. Generate builtin dump
cd giallo
npm install
node scripts/extract-grammar-metadata.js
cargo run --release --bin=build-registry --features=tools

# 3. Copy dump to giallo.kak
cp builtin.msgpack ../giallo.kak/

# 4. Build giallo-kak
cd ../giallo.kak
cargo build --release
```

## CI Matrix

The workflow builds for:
- **Linux** (x86_64-unknown-linux-gnu)
- **macOS Intel** (x86_64-apple-darwin)
- **macOS Apple Silicon** (aarch64-apple-darwin)
- **Windows** (x86_64-pc-windows-msvc)

## Release Process

To create a new release:

1. Push a tag with semantic versioning:
   ```bash
   git tag v1.0.0
   git push origin v1.0.0
   ```

2. GitHub Actions will automatically:
   - Build binaries for all platforms
   - Create a GitHub release
   - Attach the binaries as release assets

## Workflow Triggers

- **Push to main** - Builds and tests
- **Pull requests** - Runs tests
- **Tags (v\*)** - Creates releases

## Cached Dependencies

The workflow uses:
- `Swatinem/rust-cache` for Rust dependencies
- `actions/setup-node` with npm caching

This speeds up builds by caching:
- Cargo dependencies
- Build artifacts
- Node.js packages

## Local Development

For local development, you can manually build:

```bash
# Clone giallo
git clone https://github.com/getzola/giallo.git ../giallo

# Generate dump (or use existing one)
cd ../giallo
npm install
node scripts/extract-grammar-metadata.js
cargo run --release --bin=build-registry --features=tools

# Build giallo-kak
cd ../giallo.kak
cp ../giallo/builtin.msgpack .
cargo build --release
```

Or simply:
```bash
cargo build
```

If the `builtin.msgpack` is missing, the build will fail with a clear error message.
