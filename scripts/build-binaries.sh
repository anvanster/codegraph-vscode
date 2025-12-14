#!/bin/bash
# Build LSP server binaries for all platforms

set -e

echo "Building CodeGraph LSP server binaries..."

# Check if cross is installed
if ! command -v cross &> /dev/null; then
    echo "Installing cross for cross-compilation..."
    cargo install cross
fi

TARGETS=(
    "x86_64-unknown-linux-gnu"
    "x86_64-apple-darwin"
    "aarch64-apple-darwin"
    "x86_64-pc-windows-msvc"
)

# Create server directory for binaries
mkdir -p server/bin

cd server

for target in "${TARGETS[@]}"; do
    echo "Building for $target..."

    # Use cross for cross-compilation, or cargo for native
    if [[ "$(uname -s)-$(uname -m)" == "Darwin-arm64" && "$target" == "aarch64-apple-darwin" ]]; then
        cargo build --release --target "$target"
    elif [[ "$(uname -s)-$(uname -m)" == "Darwin-x86_64" && "$target" == "x86_64-apple-darwin" ]]; then
        cargo build --release --target "$target"
    elif [[ "$(uname -s)" == "Linux" && "$target" == "x86_64-unknown-linux-gnu" ]]; then
        cargo build --release --target "$target"
    else
        cross build --release --target "$target" || {
            echo "Warning: Failed to build for $target (may require additional setup)"
            continue
        }
    fi

    # Copy to bin directory with platform-specific name
    case "$target" in
        *linux*)
            cp "../target/$target/release/codegraph-lsp" "bin/codegraph-lsp-linux-x64" 2>/dev/null || true
            ;;
        x86_64*darwin*)
            cp "../target/$target/release/codegraph-lsp" "bin/codegraph-lsp-darwin-x64" 2>/dev/null || true
            ;;
        aarch64*darwin*)
            cp "../target/$target/release/codegraph-lsp" "bin/codegraph-lsp-darwin-arm64" 2>/dev/null || true
            ;;
        *windows*)
            cp "../target/$target/release/codegraph-lsp.exe" "bin/codegraph-lsp-win32-x64.exe" 2>/dev/null || true
            ;;
    esac
done

cd ..

echo "Build complete! Binaries are in server/bin/"
ls -la server/bin/ 2>/dev/null || echo "No binaries built yet"
