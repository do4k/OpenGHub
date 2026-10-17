#!/usr/bin/env bash
# Builds and installs Open G Hub for Linux: single-file binary to ~/.local/bin,
# desktop entry, and the udev HID rule.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$REPO_ROOT"

echo "==> Open G Hub installer"

# Check for the .NET SDK (10.0 or newer).
if ! command -v dotnet >/dev/null 2>&1; then
	echo "==> .NET SDK not found."
	echo "    Install the .NET 10 SDK from https://dotnet.microsoft.com/download"
	echo "    or via your distribution's packages, then re-run this script."
	exit 1
fi
echo "==> Using $(dotnet --version)"

echo "==> Publishing the single-file binary (takes a while)..."
dotnet publish src/OpenGHub.App -p:PublishProfile=linux-x64 -o bin
mv bin/OpenGHub.App bin/openghub

echo "==> Installing..."
make install

cat <<'EOF'

Done.

Unplug and replug your Logitech device so the new udev rule takes effect,
then launch Open G Hub from your application menu, or run:

    ~/.local/bin/openghub

(Add ~/.local/bin to your PATH if that command isn't found.)
EOF