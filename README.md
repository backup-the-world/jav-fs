# jav-fs

用于 JAV 媒体库的文件扫描与管理工具。

## 开发

本项目使用本地质量门禁来规范格式化、Lint、测试和提交信息。

### 开发文档

- [CLI 使用说明](docs/cli.md)：说明扫描源、扫描模式、SMB URL 和命令参数。
- [架构总览](docs/architecture.md)：说明模块边界、扫描流程、领域规则位置和常见修改入口。
- [识别规则样例](docs/recognition-rules.md)：说明番号 ID、番号前缀、分片视频和未识别视频的文件名样例。

### 前置依赖

安装本地任务运行器和提交规范工具：

```sh
cargo install just
cargo install cocogitto
```

### 初始化

安装本地 Git hooks：

```sh
just setup
```

安装后的 hooks：

- `pre-commit`：运行 `cargo fmt -- --check` 和 `cargo clippy --all-targets -- -D warnings`。
- `commit-msg`：使用 Cocogitto 校验 Conventional Commits 提交信息。

### 常用命令

```sh
just fmt        # 格式化 Rust 代码
just fmt-check  # 检查格式，不修改文件
just lint       # 运行 clippy，并把 warning 当作错误
just test       # 运行 cargo test --all-targets
just check      # 运行 fmt-check + lint + test
```

提交信息必须使用 Conventional Commits 的 `type: subject` 格式，例如：

```sh
feat: add duplicate scan summary
fix: handle empty smb host
```

常用 type 包括 `feat`、`fix`、`docs`、`style`、`refactor`、`perf`、`test`、`build`、`ci`、`chore`、`revert`。
