#!/bin/bash
set -e

# CodeGraph VS Code Extension Build Script
# Usage: ./scripts/build.sh [options]
#
# Options:
#   --clean       Clean build artifacts before building
#   --package     Create .vsix package after building
#   --install     Install the extension after packaging
#   --dev         Development build (with sourcemaps, no minification)
#   --help        Show this help message

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default options
CLEAN=false
PACKAGE=false
INSTALL=false
DEV=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --clean)
            CLEAN=true
            shift
            ;;
        --package)
            PACKAGE=true
            shift
            ;;
        --install)
            INSTALL=true
            PACKAGE=true  # Installing requires packaging
            shift
            ;;
        --dev)
            DEV=true
            shift
            ;;
        --help)
            echo "CodeGraph VS Code Extension Build Script"
            echo ""
            echo "Usage: ./scripts/build.sh [options]"
            echo ""
            echo "Options:"
            echo "  --clean       Clean build artifacts before building"
            echo "  --package     Create .vsix package after building"
            echo "  --install     Install the extension after packaging"
            echo "  --dev         Development build (with sourcemaps, no minification)"
            echo "  --help        Show this help message"
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            exit 1
            ;;
    esac
done

cd "$PROJECT_ROOT"

echo -e "${BLUE}============================================${NC}"
echo -e "${BLUE}  CodeGraph VS Code Extension Build${NC}"
echo -e "${BLUE}============================================${NC}"
echo ""

# Step 1: Clean (optional)
if [ "$CLEAN" = true ]; then
    echo -e "${YELLOW}Cleaning build artifacts...${NC}"
    rm -rf out/
    rm -rf server/target/release/
    rm -f *.vsix
    echo -e "${GREEN}✓ Clean complete${NC}"
    echo ""
fi

# Step 2: Install npm dependencies if needed
if [ ! -d "node_modules" ]; then
    echo -e "${YELLOW}Installing npm dependencies...${NC}"
    npm install
    echo -e "${GREEN}✓ Dependencies installed${NC}"
    echo ""
fi

# Step 3: Build TypeScript with esbuild
echo -e "${YELLOW}Building TypeScript extension...${NC}"
if [ "$DEV" = true ]; then
    npm run esbuild -- --sourcemap
    echo -e "${GREEN}✓ TypeScript built (development mode with sourcemaps)${NC}"
else
    npm run esbuild-base -- --production
    echo -e "${GREEN}✓ TypeScript built (production mode, minified)${NC}"
fi
echo ""

# Step 4: Build Rust LSP server
echo -e "${YELLOW}Building Rust LSP server...${NC}"
cargo build --release -p codegraph-lsp
echo -e "${GREEN}✓ Rust LSP server built${NC}"
echo ""

# Step 5: Copy binary to bin/ for packaging
echo -e "${YELLOW}Preparing binaries...${NC}"
mkdir -p bin
ARCH=$(uname -m)
OS=$(uname -s | tr '[:upper:]' '[:lower:]')

if [ "$ARCH" = "arm64" ]; then
    PLATFORM="darwin-arm64"
elif [ "$ARCH" = "x86_64" ] && [ "$OS" = "darwin" ]; then
    PLATFORM="darwin-x64"
elif [ "$ARCH" = "x86_64" ] && [ "$OS" = "linux" ]; then
    PLATFORM="linux-x64"
else
    PLATFORM="$OS-$ARCH"
fi

# Binary is at workspace root target/release/
cp target/release/codegraph-lsp "bin/codegraph-lsp-$PLATFORM"
chmod +x "bin/codegraph-lsp-$PLATFORM"
echo -e "${GREEN}✓ Binary copied to bin/codegraph-lsp-$PLATFORM${NC}"
echo ""

# Step 6: Package (optional)
if [ "$PACKAGE" = true ]; then
    echo -e "${YELLOW}Packaging extension...${NC}"
    npx vsce package --no-dependencies

    VSIX_FILE=$(ls -t *.vsix 2>/dev/null | head -1)
    if [ -n "$VSIX_FILE" ]; then
        SIZE=$(ls -lh "$VSIX_FILE" | awk '{print $5}')
        echo -e "${GREEN}✓ Package created: $VSIX_FILE ($SIZE)${NC}"
    fi
    echo ""
fi

# Step 7: Install (optional)
if [ "$INSTALL" = true ]; then
    VSIX_FILE=$(ls -t *.vsix 2>/dev/null | head -1)
    if [ -n "$VSIX_FILE" ]; then
        echo -e "${YELLOW}Installing extension...${NC}"
        code --install-extension "$VSIX_FILE" --force
        echo -e "${GREEN}✓ Extension installed${NC}"
        echo ""
        echo -e "${BLUE}Restart VS Code or run 'Developer: Reload Window' to activate.${NC}"
    else
        echo -e "${RED}✗ No .vsix file found to install${NC}"
        exit 1
    fi
fi

echo ""
echo -e "${GREEN}============================================${NC}"
echo -e "${GREEN}  Build complete!${NC}"
echo -e "${GREEN}============================================${NC}"
