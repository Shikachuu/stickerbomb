#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

CURRENT_VERSION=$(yq eval '.workspace.package.version' Cargo.toml)
NEXT_VERSION=$(git cliff --bumped-version 2>/dev/null || echo "")

if [ -z "$NEXT_VERSION" ] || [ "$CURRENT_VERSION" = "$NEXT_VERSION" ]; then
    echo "No version change needed (current: $CURRENT_VERSION)"
    exit 0
fi

echo "Bumping version from $CURRENT_VERSION to $NEXT_VERSION"

yq eval -i ".workspace.package.version = \"$NEXT_VERSION\"" Cargo.toml
cargo update -p stickerbomb -p stickerbomb-crd --quiet

yq eval -i ".version = \"$NEXT_VERSION\"" charts/stickerbomb/Chart.yaml
yq eval -i ".appVersion = \"$NEXT_VERSION\"" charts/stickerbomb/Chart.yaml

git cliff --tag "v$NEXT_VERSION" -o CHANGELOG.md
git cliff --tag "v$NEXT_VERSION" --unreleased --strip header > RELEASE_NOTES.md

if [ -n "${GITHUB_OUTPUT:-}" ]; then
    echo "version=$NEXT_VERSION" >> "$GITHUB_OUTPUT"
    echo "tag=v$NEXT_VERSION" >> "$GITHUB_OUTPUT"
fi

echo "✅ Version bumped to $NEXT_VERSION"