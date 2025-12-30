#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

VERSION=$(yq eval '.workspace.package.version' Cargo.toml)
BRANCH_NAME="release/v$VERSION"

if [ ! -f "RELEASE_NOTES.md" ]; then
    echo "Error: RELEASE_NOTES.md not found. Run release-bump-version.sh first."
    exit 1
fi

git config user.name "github-actions[bot]"
git config user.email "github-actions[bot]@users.noreply.github.com"

PR_NUMBER=$(gh pr list --label "release" --state open --json number --jq '.[0].number' || echo "")

if git ls-remote --heads origin "$BRANCH_NAME" | grep -q "$BRANCH_NAME"; then
    echo "Updating existing release branch: $BRANCH_NAME"
    git checkout -B "$BRANCH_NAME"
    git add -A
    git commit -m "chore(release): prepare for v$VERSION" || echo "No changes to commit"
    git push origin "$BRANCH_NAME" --force

    if [ -n "$PR_NUMBER" ]; then
        echo "Updating existing PR #$PR_NUMBER"
        gh pr edit "$PR_NUMBER" \
            --title "chore(release): v$VERSION" \
            --body-file RELEASE_NOTES.md
    fi
else
    echo "Creating new release branch: $BRANCH_NAME"
    git checkout -b "$BRANCH_NAME"
    git add -A
    git commit -m "chore(release): prepare for v$VERSION"
    git push origin "$BRANCH_NAME"

    echo "Creating new release PR"
    gh pr create \
        --title "chore(release): v$VERSION" \
        --body-file RELEASE_NOTES.md \
        --label "release" \
        --base main \
        --head "$BRANCH_NAME"
fi

echo "✅ Release PR ready for v$VERSION"