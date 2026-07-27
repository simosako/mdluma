#!/bin/sh

set -eu

usage() {
  printf 'usage: %s {run|test}\n' "$0" >&2
}

if [ "$#" -ne 1 ]; then
  usage
  exit 2
fi

mode=$1
case "$mode" in
  run|test) ;;
  *)
    usage
    exit 2
    ;;
esac

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
repository_root=$(CDPATH= cd -- "$script_dir/../.." && pwd -P)

if [ "$(uname -s)" != "Darwin" ]; then
  printf 'error: macOS is required to build the runtime evidence toolkit\n' >&2
  exit 1
fi

host_arch=$(uname -m)
if [ "$host_arch" != "arm64" ]; then
  printf 'error: native arm64 host required; detected %s\n' "$host_arch" >&2
  exit 1
fi

rustc=${RUSTC:-rustc}
if ! command -v "$rustc" >/dev/null 2>&1; then
  printf 'error: native rustc is required but was not found: %s\n' "$rustc" >&2
  exit 1
fi

if ! rust_cfg=$($rustc --print cfg); then
  printf 'error: rustc could not report its native compilation target\n' >&2
  exit 1
fi

rust_arch=
while IFS= read -r cfg; do
  case "$cfg" in
    'target_arch="aarch64"') rust_arch=aarch64 ;;
  esac
done <<EOF
$rust_cfg
EOF

if [ "$rust_arch" != "aarch64" ]; then
  printf 'error: rustc native target must be aarch64 for an arm64 binary\n' >&2
  exit 1
fi

build_dir=$(mktemp -d "${TMPDIR:-/tmp}/mdluma-macos-runtime-evidence.XXXXXX")
trap 'rm -rf "$build_dir"' EXIT HUP INT TERM
binary="$build_dir/macos-runtime-evidence"

case "$mode" in
  run)
    "$rustc" --edition=2021 "$repository_root/tools/macos-runtime-evidence/main.rs" -o "$binary"
    "$binary"
    ;;
  test)
    "$rustc" --edition=2021 --test "$repository_root/tools/macos-runtime-evidence/main.rs" -o "$binary"
    "$binary"
    ;;
esac
