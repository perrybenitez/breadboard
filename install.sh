#!/bin/bash

set -e

echo "🔧 Building Breadboard..."
cargo build --release

echo ""
echo "📦 Installing breadboard to /usr/local/bin..."

if [ -w /usr/local/bin ]; then
    cp target/release/breadboard /usr/local/bin/
else
    sudo cp target/release/breadboard /usr/local/bin/
fi

echo ""
echo "✅ Breadboard installed successfully!"
echo ""
echo "Get started:"
echo "  breadboard status              # View current usage"
echo "  breadboard set-budget claude 2000000"
echo "  breadboard \"your prompt here\""
echo ""
echo "For more info: breadboard --help"
