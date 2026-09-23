#!/usr/bin/env bash
set -euo pipefail
profile="${1:-debug}"
host="$(rustc -vV | sed -n 's/^host: //p')"
case "$host" in *-unknown-linux-gnu) ;; *) echo "unsupported Linux sidecar host: $host" >&2; exit 1 ;; esac
case "$profile" in debug) args=() ;; release) args=(--release) ;; *) echo "usage: $0 [debug|release]" >&2; exit 2 ;; esac
cargo build -p nyrva-hook --locked "${args[@]}"
cargo build -p nyrva-core --bin nyrva-telemetry --locked "${args[@]}"
mkdir -p nyrva/binaries
for binary in nyrva-hook nyrva-telemetry; do
  source_bin="target/$profile/$binary"
  destination="nyrva/binaries/$binary-$host"
  cp "$source_bin" "$destination"
  chmod 0755 "$destination"
  echo "prepared $destination from $source_bin"
done
