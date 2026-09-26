#!/usr/bin/env bash
set -euo pipefail

readonly default_ref="b9d2627f83d189f0e7447b6bc0820f95bd061749"
ref="${1:-${TRUSTIFY_REF:-$default_ref}}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
destination="$repo_root/openapi/openapi.yaml"
temporary_directory="$(mktemp -d)"
trap 'rm -rf "$temporary_directory"' EXIT

git -C "$temporary_directory" init --quiet
git -C "$temporary_directory" remote add origin https://github.com/guacsec/trustify.git
git -C "$temporary_directory" fetch --quiet --depth 1 --filter=blob:none origin "$ref"
git -C "$temporary_directory" show FETCH_HEAD:openapi.yaml > "$temporary_directory/openapi.yaml"

if ! grep -q '^openapi: 3\.' "$temporary_directory/openapi.yaml"; then
    printf 'Fetched file does not look like an OpenAPI document at ref %s\n' "$ref" >&2
    exit 1
fi

install -m 0644 "$temporary_directory/openapi.yaml" "$destination"
printf 'Synced Trustify ref %s to %s\n' "$ref" "$destination"
if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$destination"
else
    shasum -a 256 "$destination"
fi
