#!/bin/bash

set -e  # Exit on error

# Install required tools
echo "Installing required tools..."
sudo apt-get update
sudo apt-get install -y musl-tools mingw-w64 curl clang llvm-dev libclang-dev

# Install Rust if not already installed
if ! command -v rustc &> /dev/null; then
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
fi

# Install required Rust targets
echo "Installing Rust targets..."
rustup target add x86_64-unknown-linux-musl
rustup target add x86_64-pc-windows-gnu

# Set environment variables for musl build
export BINDGEN_EXTRA_CLANG_ARGS="-I/usr/include/x86_64-linux-musl"
export CC=musl-gcc
export AR=ar

# Build for Linux with musl
echo "Building vn-link-cli for Linux with musl..."
cargo build --release --target x86_64-unknown-linux-musl -p vn-link-cli

# Copy Linux binary
mkdir -p dist
cp target/x86_64-unknown-linux-musl/release/vn-link-cli dist/vn-link-cli-linux-x86_64

# Check Linux build
if [ -f "dist/vn-link-cli-linux-x86_64" ]; then
    echo "Linux build complete! Binary is in the dist directory."
    echo "Linux binary info:"
    file dist/vn-link-cli-linux-x86_64
else
    echo "Linux build failed."
    exit 1
fi

# Build for Windows
echo "Building for Windows..."
export BINDGEN_EXTRA_CLANG_ARGS="-I/usr/x86_64-w64-mingw32/include"
export CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc
export AR_x86_64_pc_windows_gnu=x86_64-w64-mingw32-ar
export RUSTFLAGS="-C linker=x86_64-w64-mingw32-gcc"

echo "Building vn-link-cli for Windows..."
cargo build --release --target x86_64-pc-windows-gnu -p vn-link-cli

# Copy Windows binary
cp target/x86_64-pc-windows-gnu/release/vn-link-cli.exe dist/vn-link-cli-windows-x86_64.exe

# Check Windows build
if [ -f "dist/vn-link-cli-windows-x86_64.exe" ]; then
    echo "Windows build complete! Binary is in the dist directory."
    echo "Windows binary info:"
    file dist/vn-link-cli-windows-x86_64.exe
else
    echo "Windows build failed."
    exit 1
fi

echo "Build process completed successfully." 