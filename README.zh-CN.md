# Stillrun

Stillrun 是一个面向 macOS 的命令生命周期运行时（Command Lifecycle Runtime）。
它不是另一个 shell history，也不是另一个 PM2。它的核心想法是：终端里的每一条命令都不应该只是“一次执行”，而应该可以成为一个带有历史、日志、状态、重放和生命周期的 Job。

第一阶段只做命令行工具，不做 TUI、不做 Web UI，也不引入 AI。后台持久化优先复用 macOS 原生的 launchd，不重新造一个常驻 daemon。

## 当前已经做到

- `stillrun run -- <command...>`：前台执行命令，并记录命令、argv、cwd、Git 仓库、Git 分支、开始/结束时间、耗时、退出状态、stdout、stderr、PID 和脱敏后的环境信息。
- `stillrun history`：查看 Stillrun 已记录的命令历史。
- `stillrun history --query <text>`：用 SQLite FTS5 搜索命令文本和已捕获输出。
- `stillrun history --cwd <path>`、`--repo <path>`、`--status <status>`、`--since-ms`、`--until-ms`：按目录、仓库、执行结果和时间范围过滤历史。
- `stillrun import-history --shell auto`：在用户明确同意后，把本机已有的 zsh、bash、fish history 导入 Stillrun。
- `stillrun replay <id>`：按记录里的 cwd 和非敏感环境变量重放命令。
- `stillrun run --background --name <name> -- <command...>`：把普通命令变成 launchd 管理的后台 Job。
- `stillrun promote <id> --name <name>`：把历史命令提升成后台 Job。
- `stillrun jobs`：列出 Job，并尽量同步 launchd 的运行态、PID、CPU、RSS、退出码和重启次数。
- `stillrun status <job>`：查看单个 Job 的状态和日志路径。
- `stillrun logs <job> --follow`：查看或跟随 Job 的 stdout/stderr 日志。
- `stillrun start <job>`、`stillrun stop <job>`、`stillrun restart <job>`：统一管理 Job 生命周期。
- 敏感信息脱敏：Token、密码、Secret、API Key、Bearer Token、`token=...`、`--token xxx`、`--password=xxx` 等会在写入 SQLite 前脱敏。

## 为什么你现在 `stillrun history` 是空的

Stillrun 不会直接读取 shell 的历史文件。它默认只查询自己的 SQLite 数据库：

```text
~/Library/Application Support/Stillrun/stillrun.db
```

所以如果你过去没有通过 `stillrun run -- ...` 执行命令，也没有导入本机历史，那么：

```sh
stillrun history
stillrun history --query "npm"
stillrun history --query "site-pilot"
```

都会没有结果。这不是搜索坏了，而是数据还没有进入 Stillrun。

导入一次即可：

```sh
stillrun import-history --shell auto
stillrun history --query "npm"
stillrun history --query "lxy-1"
```

导入是幂等的：重复导入同一个 history 文件时，Stillrun 会根据来源文件和行号跳过已导入记录。

如果旧版本导入过乱码记录，升级后重新执行导入即可刷新同一来源的记录：

```sh
stillrun import-history --shell auto
```

Stillrun 会逐行处理 zsh history 的原生转义，并尝试 UTF-8、GB18030/GBK 等常见编码，尽量避免中文命令变成乱码或 `�`。

## 安装

推荐使用安装脚本：

```sh
./scripts/install.sh
```

脚本会执行：

```sh
cargo install --path . --force
```

安装完成后，如果当前是交互式终端，会询问是否导入本机已有 shell history。只有你输入 `y` / `yes` 后才会读取本机 history 文件并写入 Stillrun 的本地 SQLite。

如果你不想要交互提示，也可以直接安装：

```sh
cargo install --path .
```

## 常用命令

记录一条前台命令：

```sh
stillrun run -- npm run dev
```

搜索历史：

```sh
stillrun history --query "npm"
stillrun history --cwd /path/to/project
stillrun history --status success
stillrun history --status imported --query "site-pilot"
```

导入本机历史：

```sh
stillrun import-history --shell auto
stillrun import-history --shell zsh --file ~/.zsh_history
stillrun import-history --shell bash --file ~/.bash_history
stillrun import-history --shell fish --file ~/.local/share/fish/fish_history
```

重放历史命令：

```sh
stillrun replay 1
```

把历史命令提升成后台 Job：

```sh
stillrun run -- python scripts/download.py
stillrun history --query download.py
stillrun promote 1 --name downloader
stillrun logs downloader --follow
```

创建和管理后台 Job：

```sh
stillrun run --background --name dev-server -- npm run dev
stillrun run --background --keep-alive --name api-server -- cargo run
stillrun jobs
stillrun status dev-server
stillrun logs dev-server --follow
stillrun stop dev-server
stillrun start dev-server
stillrun restart dev-server
```

如果命令需要管道、重定向、通配符或复合 shell 语法，请显式调用 shell：

```sh
stillrun run -- zsh -lc "curl -s https://example.com | jq ."
```

## 存储位置

默认数据目录：

```text
~/Library/Application Support/Stillrun/stillrun.db
~/Library/Application Support/Stillrun/logs/
~/Library/Application Support/Stillrun/config.toml
```

后台 Job 的 launchd plist 默认写入：

```text
~/Library/LaunchAgents/
```

开发或测试时可以隔离状态：

```sh
STILLRUN_HOME=/tmp/stillrun-dev cargo run -- history
```

## 架构

- CLI：`clap`
- 异步运行时：`tokio`
- 存储：SQLite + FTS5（通过 `rusqlite`）
- 配置：`serde` + TOML
- 日志：`tracing`
- 文件监听扩展点：`notify`
- 进程执行：`tokio::process`
- macOS 后台持久化：launchd + `launchctl`

代码按生命周期职责拆分：

- `src/execution.rs`：前台执行和 replay。
- `src/history_import.rs`：导入 zsh/bash/fish 历史。
- `src/db.rs`：SQLite schema、history、Job 和 FTS 搜索。
- `src/context.rs`：cwd、Git 和环境捕获。
- `src/redact.rs`：写入前脱敏。
- `src/jobs/`：launchd plist、bootstrap/bootout、运行状态和资源采样。
- `src/cli.rs`：用户可见的命令行入口。

## 还没做到

- 还没有 TUI 或 Web UI。
- 还没有 AI 搜索、命令模板、远程节点管理或 Agent Timeline。
- 还没有 Linux / Windows 后台生命周期实现；MVP 只专注 macOS。
- 从已有 shell history 导入的记录没有原始 cwd、Git 分支、退出码、stdout/stderr，因为传统 shell history 本身不保存这些上下文。导入后可以搜索，也可以通过对应 shell replay，但它不是完整的 Stillrun 原生执行记录。
- 暂时没有持续资源监控采样时间线；当前 `jobs` / `status` 是按需同步 launchd 和 `ps` 的即时状态。
- 还没有打包成 Homebrew formula、pkg 安装器或签名发布包。

## 安全边界

Stillrun 会在写入 SQLite 前脱敏常见敏感信息，但它不是密码管理器，也不能保证识别所有自定义 secret 格式。

Replay 会清空当前进程环境，然后恢复记录里的非敏感环境变量。被脱敏的 secret 不会被重新注入。

Promote 和后台 Job 也遵循同样规则：launchd plist 只写入非敏感环境变量。
