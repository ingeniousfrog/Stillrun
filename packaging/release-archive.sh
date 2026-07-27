#!/usr/bin/env bash
set -euo pipefail

target="${1:?usage: packaging/release-archive.sh <rust-target>}"
archive="stillrun-${target}.tar.gz"
dist_dir="${DIST_DIR:-dist}"
binary_path="target/${target}/release/stillrun"

cargo build --locked --release --target "${target}"

if [[ ! -x "${binary_path}" ]]; then
  echo "Release binary not found or not executable: ${binary_path}" >&2
  exit 1
fi

staging_dir="$(mktemp -d)"
cleanup() {
  rm -rf "${staging_dir}"
}
trap cleanup EXIT

cp "${binary_path}" "${staging_dir}/stillrun"
cp README.md README.zh-CN.md LICENSE "${staging_dir}/"

mkdir -p "${dist_dir}"
tar -C "${staging_dir}" -czf "${dist_dir}/${archive}" stillrun README.md README.zh-CN.md LICENSE
shasum -a 256 "${dist_dir}/${archive}" | tee "${dist_dir}/${archive}.sha256"
