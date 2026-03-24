#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")"

echo "Building..."
cargo build --release

cp target/release/claude-fn-length-hook ~/bin/claude-fn-length-hook
echo "Installed claude-fn-length-hook to ~/bin/"
