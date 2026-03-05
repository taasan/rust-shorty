#!/usr/bin/env bash
set -euo pipefail

TARGET=x86_64-unknown-linux-musl

binaries=$(cargo build --release --target "$TARGET" --message-format=json \
| jq -r '
    select(.reason=="compiler-artifact")
    | select(.target.kind | index("bin"))
    | select(.target.kind | index("test") | not)
    | select(.target.kind | index("example") | not)
    | .executable
  ')

printf '%s\n' "$binaries"
printf %s "$binaries" | xargs --no-run-if-empty upx --best --lzma
