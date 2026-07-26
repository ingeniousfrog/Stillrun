# Stillrun

Stillrun 是一个面向 macOS 的命令生命周期运行时。它会记录终端命令，让你可以搜索、检查、重放，并把常用命令提升成 launchd 管理的后台 Job，带日志和状态。

## 安装

```sh
./scripts/install.sh
```

安装脚本会执行 `cargo install --path . --force`。如果你同意，它还会导入本机 shell history，并安装 shell hook。

非交互安装：

```sh
cargo install --path .
```

## 快速帮助

```sh
stillrun -h
stillrun <command> -h
```

`stillrun -h` 会直接展示能力列表和简短示例；子命令帮助用于查看某个工作流的具体参数。

## 能力列表

| 能力 | 命令 | 示例 |
| --- | --- | --- |
| 运行并记录 | `run` | `stillrun run -- npm run dev` |
| 搜索历史 | `history` | `stillrun history --query "npm" --sort oldest` |
| 导入 shell history | `import-history` | `stillrun import-history --shell auto --preview` |
| 安全重放 | `replay` | `stillrun replay 12 --preview` |
| 提升为 Job | `promote` | `stillrun promote 12 --name dev-server` |
| 管理 Job | `jobs`、`status`、`start`、`stop`、`restart` | `stillrun status dev-server` |
| 查看日志 | `logs` | `stillrun logs dev-server --follow` |
| 检查元数据 | `inspect` | `stillrun inspect 12 --json` |
| 管理配置 | `config` | `stillrun config set max-output-bytes 2097152` |
| shell hook | `hook` | `stillrun hook install --shell auto` |
| shell completion | `completion` | `stillrun completion zsh > ~/.stillrun-completion.zsh` |

## 常用示例

运行并记录前台命令：

```sh
stillrun run -- npm run dev
stillrun run -- zsh -lc "curl -s https://example.com | jq ."
```

搜索和检查历史：

```sh
stillrun history --query "npm"
stillrun history --query "咖啡厅"
stillrun history --status success
stillrun history --status imported --query "cargo"
stillrun history --since-ms 1720000000000 --until-ms 1730000000000
stillrun history --sort oldest --full --details
stillrun inspect 1 --json
```

先预览，再导入本机 shell history：

```sh
stillrun import-history --shell auto --preview
stillrun import-history --shell auto
stillrun import-history --shell zsh --file ~/.zsh_history --yes
```

重放命令：

```sh
stillrun replay 1 --preview
stillrun replay 1 --yes
```

创建和管理后台 Job：

```sh
stillrun run --background --name dev-server -- npm run dev
stillrun run --background --keep-alive --name api-server -- cargo run
stillrun jobs
stillrun status dev-server
stillrun logs dev-server --follow
stillrun jobs monitor dev-server --once --cpu-alert 90 --rss-alert-mb 1024
stillrun stop dev-server
stillrun start dev-server
stillrun restart dev-server
stillrun jobs delete dev-server
```

把历史命令提升成后台 Job：

```sh
stillrun run -- python scripts/download.py
stillrun history --query download.py
stillrun promote 1 --name downloader
stillrun logs downloader --follow
```

管理配置：

```sh
stillrun config show
stillrun config show --json
stillrun config path
stillrun config set max-output-bytes 2097152
stillrun config redact add session_token
stillrun config redact list
```

安装辅助功能：

```sh
stillrun hook install --shell auto
stillrun hook print --shell zsh
stillrun completion zsh > ~/.stillrun-completion.zsh
printf '\nsource ~/.stillrun-completion.zsh\n' >> ~/.zshrc
```

## 存储位置

Stillrun 的本地状态默认写在：

```text
~/Library/Application Support/Stillrun/stillrun.db
~/Library/Application Support/Stillrun/logs/
~/Library/Application Support/Stillrun/config.toml
```

后台 Job 的 plist 默认写入 `~/Library/LaunchAgents/`。

开发或测试时可以用 `STILLRUN_HOME` 隔离状态：

```sh
STILLRUN_HOME=/tmp/stillrun-dev cargo run -- history
```

## 安全说明

Stillrun 写入 SQLite 前会脱敏常见敏感环境变量和命令/输出里的 secret 模式，包括 token、password、secret、API key、`Authorization: Bearer ...`、`token=...`、`--token value`、`--password=value` 等。

Replay 会清空当前进程环境，再恢复记录里的非敏感环境变量。从 shell history 导入的命令在 replay 前会要求 preview 或确认，避免误重放旧命令。

## 测试

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```

真实 macOS launchd 生命周期 E2E 需要显式打开开关，因为它会短暂触碰用户 launchd 会话：

```sh
STILLRUN_RUN_LAUNCHD_E2E=1 cargo test --test launchd_e2e -- --nocapture
```
