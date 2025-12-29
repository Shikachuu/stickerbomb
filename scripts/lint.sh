#!/usr/bin/env bash
set -euo pipefail

echo "✏️ Formatting with fmt..."
cargo fmt --all

echo "👀 Linting Rust codebase..."
cargo clippy --all --all-targets --all-features -- -D warnings

echo "👀 Linting helm chart..."
helm lint "$CHART_DIR"

echo "👀 Validating templated helm chart with kubeconform..."
helm template "$CHART_DIR" | kubeconform -schema-location default -schema-location schemas/{{.ResourceKind}}_{{.ResourceAPIVersion}}.json

echo "👀 Running license check..."
cargo deny check

echo "✏️ Writing file headers..."
addlicense -l "apache" -s=only -c "Stickerbomb Maintainers" -ignore "**/*.toml" "crates"
