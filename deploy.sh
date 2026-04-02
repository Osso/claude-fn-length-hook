#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")"

echo "Installing..."
cargo install --path .

echo "Installed claude-fn-length-hook to ~/.cargo/bin/"
