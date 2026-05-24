# Agent 指南 - jav-fs

本仓库包含 `jav-fs`：一个基于 Rust 的视频文件扫描与管理工具，重点用于提取番号 ID，并处理 SMB/UNC 路径。它的设计目标是快速、多线程，并能稳健处理多种文件名格式。

## Git

- 未经明确许可，不要提交更改。
- 提交信息使用 Conventional Commits，格式为 `type: subject`，例如 `feat: add scanner option`。
- 常用提交类型：`feat`、`fix`、`docs`、`style`、`refactor`、`perf`、`test`、`build`、`ci`、`chore`、`revert`。

## 构建、Lint 与测试命令

所有操作都使用标准 Cargo 命令。建议在项目根目录执行。

### 构建
- **构建项目：** `cargo build`
- **构建 release 版本：** `cargo build --release`（实际使用时推荐，性能更好）

### 本地工程规范入口
- **安装本地 hooks：** `just setup`（需要 `cog`/Cocogitto）
- **完整本地门禁：** `just check`（格式检查 + clippy + 测试）
- **提交前检查：** `just pre-commit`（与 pre-commit hook 一致）

### Lint 与格式化
- **检查格式：** `just fmt-check`（等价于 `cargo fmt -- --check`）
- **应用格式化：** `just fmt`（等价于 `cargo fmt`）
- **运行 clippy：** `just lint`（等价于 `cargo clippy --all-targets -- -D warnings`）
- **修复 clippy 建议：** `cargo clippy --fix`

### 测试
- **运行全部测试：** `just test`（等价于 `cargo test --all-targets`）
- **运行指定测试：** `cargo test <test_function_name>`
  - 示例：`cargo test test_convert_smb_url_to_unc_basic`
- **运行某个模块的测试：** `cargo test tests::<module_name>`
- **运行测试并显示输出：** `cargo test -- --nocapture`（调试 `println!` 时有用）

### 运行应用
- **从源码执行：** `cargo run -- <URL> [ARGS]`
- **示例：** `cargo run -- smb://nas/video --threads 4`

## 代码风格指南

### 文档语言
- 本项目的文档默认使用中文，包括 README、PRD、Issue、ADR、Agent 指南、领域文档和普通 Markdown 说明。
- 英文仅用于专业术语、代码标识符、命令、配置项、协议名、库名、错误原文、Conventional Commits 类型以及难以准确翻译的领域词汇。
- 如需保留英文术语，优先采用“中文说明 + 英文术语”的形式，例如“重复扫描（duplicate scan）”、“分片视频（video part）”。
- 面向用户的新增说明、注释性文档和 issue 描述应避免整段英文，除非引用外部原文或 API/CLI 输出。

### 语言与 Edition
- **Rust：** 使用 `Cargo.toml` 中指定的 Edition 2021。避免使用已废弃特性或旧 edition 写法。

### Imports
- 按以下顺序分组导入，并在组之间留空行：
  1. 标准库（`std::...`）
  2. 外部依赖（`clap`、`dashmap`、`url` 等）
  3. 本地模块导入（`use crate::...` 或 `use jav_fs::...`）
- 优先使用显式导入（例如 `use std::sync::Arc`），避免通配符导入（`use std::sync::*`）。测试模块中标准的 `use super::*;` 例外。

### 格式化
- 严格遵循 `rustfmt` 默认格式。
- 最大行宽通常保持在 100-120 字符左右，但以 `cargo fmt` 自动处理为准。
- 使用 4 个空格缩进。

### 命名约定
- **变量/函数/模块：** `snake_case`（例如 `extract_id_from_filename`、`scan_path`）。
- **结构体/枚举/Trait：** `PascalCase`（例如 `Args`、`WalkState`）。
- **常量：** `SCREAMING_SNAKE_CASE`。
- **布尔值：** 适当使用 `is_`、`has_`、`can_` 前缀（例如 `is_video_file`、`has_auth`）。

### 类型与所有权
- 拥有文本数据时使用 `String`，只读字符串切片使用 `&str`。
- 局部变量可利用 Rust 类型推断；复杂泛型结构（如 `Arc<DashMap<String, String>>`）或公开 API 签名应提供显式类型。
- **线程安全：** 本项目使用 `Arc<T>` 做共享所有权，`AtomicUsize` 做共享计数器，并使用 `DashMap` 等线程安全集合做并发存储。除非确有必要处理复杂状态同步，否则避免使用 `Mutex<T>`。

### 错误处理
- 可恢复错误使用 `Result<T, E>`，可选值使用 `Option<T>`。
- 优先使用 `map_err` 转换错误类型或补充上下文字符串。
  - 示例：`Url::parse(url).map_err(|e| format!("Failed to parse URL: {}", e))?`
- 错误信息应简洁、描述清楚，并以大写字母开头。
- 谨慎使用 `unwrap()` 或 `expect()`。它们可用于：
  - 单元测试。
  - 初始化已知合法的 `Regex`。
  - 确实不可能失败的场景（例如刚确认是文件后获取文件名）。

### 正则表达式
- 正则表达式用于识别视频文件和提取 ID。
- 当前代码在函数内调用 `Regex::new()`。如果函数处于扫描等高频路径中，可考虑把 `Regex` 移到 `once_cell::sync::Lazy` 或 `lazy_static!` 以提升性能。
- ID 提取模式：`r"[[:alpha:]]+-\d+|[[:alpha:]]+\d+"`（匹配带横杠或不带横杠的字母数字 ID）。

### 文档与注释
- 对公开函数、结构体和模块使用 `///` 文档注释。简要说明用途、参数和返回值。
- 内部实现说明使用 `//`。
- 注释应重点解释“为什么”（意图），而不是复述代码“做了什么”，尤其是非显而易见的逻辑。
- 不要保留“注释掉的代码”；直接删除。

## 项目结构与架构

### 文件
- `src/main.rs`：入口点。处理 CLI 参数解析（使用 `clap`）、SMB 认证（Windows 上使用 `net use`），并编排扫描流程。
- `src/lib.rs`：核心逻辑库。包含 URL 转换工具、文件名过滤器和 ID 提取逻辑。大多数单元测试位于此处。
- `Cargo.toml`：项目元数据与依赖。

### 并发模型
- 扫描器使用 `ignore::WalkBuilder` 的 `build_parallel()` 并发遍历文件系统。
- 结果收集到 `Arc<DashMap<String, String>>`，避免全局锁并尽量提升吞吐。
- 进度通过 `indicatif::ProgressBar` 报告，并由多个线程通过原子计数器更新。

### SMB 处理
- SMB URL（`smb://host/share`）会转换为 UNC 路径（`\\host\share`），以便 Windows 原生文件访问。
- 如果 URL 中提供了凭据，则通过执行 `net use` 处理认证。

## Agent 开发流程
1. **分析：** 修改前先阅读 `src/lib.rs` 和 `src/main.rs`，理解现有模式。
2. **实现：** 按上述指南编写惯用 Rust 代码。
3. **测试：** 新逻辑需在 `src/lib.rs` 中添加单元测试。运行 `cargo test`。
4. **Lint：** 完成前运行 `cargo clippy` 和 `cargo fmt`。
5. **验证：** 如果改动影响 CLI，使用 `cargo run -- <args>` 运行应用确认行为正确。

## Agent 技能

### Issue tracker

Issue 和 PRD 以本地 markdown 文件形式存放在 `.scratch/` 下。详见 `docs/agents/issue-tracker.md`。

### Triage labels

Triage 角色使用默认的标签/状态词汇。详见 `docs/agents/triage-labels.md`。

### Domain docs

本仓库使用 single-context 的领域文档布局。详见 `docs/agents/domain.md`。
