#!/bin/bash
# CI Checks - Run this before pushing to ensure CI passes

set -e  # Exit on first error

echo "============================================"
echo "Running CI checks locally..."
echo "============================================"
echo ""

# Function to print section headers
print_section() {
    echo ""
    echo "============================================"
    echo "$1"
    echo "============================================"
}

# Track timing
START_TIME=$(date +%s)

# 1. TypeScript lint
print_section "1/6 Running ESLint..."
npm run lint

# 2. TypeScript compile
print_section "2/6 Compiling TypeScript..."
npm run compile

# 3. TypeScript tests
print_section "3/6 Running TypeScript tests..."
npm test

# 4. Rust format check
print_section "4/6 Checking Rust formatting..."
cargo fmt --all -- --check

# 5. Rust clippy
print_section "5/6 Running Clippy..."
cargo clippy --all-targets --all-features -- -D warnings

# 6. Rust build & test
print_section "6/6 Building and testing Rust..."
cargo build --release
cargo test --all-features

# Calculate elapsed time
END_TIME=$(date +%s)
ELAPSED=$((END_TIME - START_TIME))

echo ""
echo "============================================"
echo "✅ All CI checks passed! (${ELAPSED}s)"
echo "============================================"
echo ""
echo "Ready to push. To release:"
echo "  git tag v0.1.0"
echo "  git push origin v0.1.0"
