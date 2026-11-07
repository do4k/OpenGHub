#!/usr/bin/env bash
# Builds and installs Open G Hub for Linux: release binary to ~/.local/bin,
# desktop entry, and the udev HID rule.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$REPO_ROOT"

echo "==> Open G Hub installer"

# Check for the Rust toolchain (1.99 or newer, pinned in rust-toolchain.toml).
if ! command -v cargo >/dev/null 2>&1; then
	echo "==> Rust toolchain not found."
	echo "    Install Rust from https://rustup.rs, then re-run this script."
	exit 1
fi
echo "==> Using $(rustc --version)"

echo "==> Building the release binary (takes a while)..."
cargo build --release -p openghub-app
mkdir -p bin
cp target/release/openghub-app bin/openghub

echo "==> Installing..."
make install

cat <<'EOF'

Done.

Unplug and replug your Logitech device so the new udev rule takes effect,
then launch Open G Hub from your application menu, or run:

    ~/.local/bin/openghub

(Add ~/.local/bin to your PATH if that command isn't found.)
EOF
