#!/bin/bash
set -e

VERSION=$1
if [ -z "$VERSION" ]; then
  echo "Usage: ./release.sh <version>"
  exit 1
fi

# Bump version in Cargo.toml
sed -i '' "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml

# Regenerate Cargo.lock
cargo build

# Commit and tag
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "chore: release v$VERSION"
git tag "v$VERSION"

# Push
git push && git push --tags

echo "Released v$VERSION"
