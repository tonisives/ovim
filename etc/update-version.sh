#!/bin/bash

# Script to update version numbers across the project when releasing with v-prefix
# Usage: ./update-version.sh <tag_name>

set -e

TAG_NAME="${1:-$GITHUB_REF_NAME}"

echo "Release tag: $TAG_NAME"

if [[ "$TAG_NAME" =~ ^v ]]; then
    # Extract version by removing 'v' prefix
    VERSION="${TAG_NAME:1}"
    echo "Extracted version: $VERSION"

    # Update package.json version
    echo "Updating package.json to version $VERSION"
    pnpm version --no-git-tag-version "$VERSION"

    # Update tauri.conf.json version
    echo "Updating tauri.conf.json to version $VERSION"
    sed -i.bak "s/\"version\": \"[^\"]*\"/\"version\": \"$VERSION\"/" src-tauri/tauri.conf.json
    rm -f src-tauri/tauri.conf.json.bak

    # Update Cargo.toml version
    echo "Updating Cargo.toml version to $VERSION"
    sed -i.bak "s/^version = .*/version = \"$VERSION\"/" src-tauri/Cargo.toml
    rm -f src-tauri/Cargo.toml.bak

    echo "Successfully updated all version files to: $VERSION"

    # In CI environment, commit the changes
    if [[ -n "$GITHUB_ACTIONS" ]]; then
        git config --global user.name "github-actions[bot]"
        git config --global user.email "github-actions[bot]@users.noreply.github.com"
        git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml
        git commit -m "chore: update version to $VERSION" || true
        git push origin HEAD || true
        echo "Committed version changes for release $TAG_NAME"
    fi
else
    echo "Tag '$TAG_NAME' doesn't start with 'v', skipping version update"
    exit 0
fi
