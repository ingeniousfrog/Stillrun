#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$repo_root"
cargo install --path . --force

if [[ ! -t 0 ]]; then
  echo "Non-interactive install detected; skip shell history import."
  exit 0
fi

printf "是否将本机已有 shell history 导入 Stillrun？这只会读取本机 history 文件并写入 Stillrun 本地 SQLite。[y/N] "
read -r answer

case "${answer:-}" in
  y|Y|yes|YES)
    stillrun import-history --shell auto --yes
    ;;
  *)
    echo "Skipped shell history import. You can run: stillrun import-history --shell auto"
    ;;
esac

printf "是否安装 Stillrun shell hook，以便未来命令自动记录 cwd/Git/退出码？[y/N] "
read -r hook_answer

case "${hook_answer:-}" in
  y|Y|yes|YES)
    stillrun hook install --shell auto
    ;;
  *)
    echo "Skipped shell hook install. You can run: stillrun hook install --shell auto"
    ;;
esac
