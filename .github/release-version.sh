#!/usr/bin/env bash
# Prints version=X.Y.Z for GitHub Actions from a tag like vX.Y.Z, and fails
# if it does not match the workspace version in Cargo.toml.
set -euo pipefail
tag="${1:?usage: release-version.sh vX.Y.Z}"
version="${tag#v}"
cargo_version="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n1)"
if [ "$version" != "$cargo_version" ]; then
    echo "::error::tag $tag does not match version $cargo_version in Cargo.toml" >&2
    exit 1
fi
echo "version=$version" >> "${GITHUB_OUTPUT:-/dev/stdout}"
