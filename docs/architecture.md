# 架构总览

本文面向后续开发者，说明 `jav-fs` 在 JAV 文件管理上下文中的模块边界、调用关系和主要数据流。

## 当前边界

```text
src/main.rs  应用编排层：CLI、扫描模式选择、并发遍历、进度展示、结果输出
src/lib.rs   领域规则层：SMB 路径转换、文件类型识别、番号 ID/番号前缀/分片视频识别、入库配置解析、入库源扫描、目标路径规划、NFO 生成、图片准备、移动执行和结构化报告
```

`src/main.rs` 不应沉淀复杂的文件名识别规则；这类规则应优先放在 `src/lib.rs`，并用单元测试覆盖。

## 启动流程

```text
main
├── 解析 CLI 参数
├── 解析扫描路径
│   └── SMB URL 转 UNC 路径
├── 如需要则执行 SMB 认证
└── 选择扫描模式
    ├── 统计扫描
    ├── 重复扫描
    ├── 前缀扫描
    └── 入库整理 dry-run
```

## 扫描模型

所有扫描模式都基于 `ignore::WalkBuilder` 并发遍历扫描路径。遍历过程只处理普通文件，目录和遍历错误会被跳过。

共享状态使用线程安全结构收集：

- 计数器使用 `Arc<AtomicUsize>`。
- 唯一集合和映射使用 `Arc<DashMap<...>>`。
- 重复候选列表当前使用 `Arc<Mutex<Vec<String>>>`。

## 统计扫描

统计扫描用于获得 JAV 媒体库的规模概览。

```text
扫描路径
→ 遍历普通文件
→ 统计文件总数
→ 识别视频文件并汇总数量和大小
→ 识别作品辅助媒体并汇总数量和大小
→ 输出统计结果
```

相关领域规则：

- `is_video_file`
- `is_image_file`

视频文件定义集中在 `src/lib.rs`，当前统一支持 `.mp4`、`.mkv`、`.wmv`、`.avi`、`.mov`、`.m4v`、`.ts`，统计扫描、重复扫描、前缀扫描和后续入库整理都应复用这一规则。

## 重复扫描

重复扫描用于发现重复候选。

```text
扫描路径
→ 遍历普通文件
→ 只处理视频文件
→ 从文件名提取番号 ID
    ├── 提取成功：按大写番号 ID 聚合同一 JAV 作品的视频文件
    │   ├── 首次出现：记录为该 JAV 作品的代表文件
    │   └── 再次出现：如果不是不同分片视频，则记录为重复候选
    └── 提取失败：记录为未识别视频
→ 输出视频总数、已识别作品数、未识别视频数、重复候选大小和路径
```

相关领域规则：

- `is_video_file`
- `extract_id_from_filename`
- `is_distinct_video_part`
- `extract_video_part_from_filename`

注意：重复候选不是“确认可删除的重复文件”。当前判断只基于番号 ID 和分片视频规则，没有比较文件内容、字幕版本、清晰度或其他版本差异。

## 前缀扫描

前缀扫描用于盘点 JAV 媒体库中出现过的番号前缀。

```text
扫描路径
→ 遍历普通文件
→ 只处理视频文件
→ 提取番号 ID
→ 提取番号前缀
→ 去重、排序并输出
```

相关领域规则：

- `is_video_file`
- `extract_id_from_filename`
- `extract_prefix_from_id`

## 入库整理 dry-run

入库整理通过 `organize` 子命令启动。当前阶段只实现安全 dry-run：解析完整 CLI 或完整本地配置，扫描 source 下的非隐藏路径，跳过默认排除目录和用户追加 exclude，识别统一视频扩展名范围内的候选视频，沿用番号 ID 提取规则，并查询 SQLite 数据库 `videos.product_id` 判断是否缺少元数据。

```text
organize 配置
→ 解析 source/target/database/exclude
→ 递归扫描 source
→ 跳过隐藏路径、@eaDir、tmp、temp、incomplete 和用户 exclude
→ 识别视频候选
    ├── 无法提取番号 ID：未识别视频
    └── 可提取番号 ID：查询 videos.product_id
        ├── 存在：有基础元数据的候选
        └── 不存在：缺少元数据
→ 输出基础 dry-run 报告
```

相关领域规则和入口：

- `load_organize_options`
- `scan_organize_source`
- `run_organize_dry_run`
- `plan_organize_targets`
- `generate_nfo`
- `prepare_artwork`
- `apply_planned_videos`
- `OrganizeDryRunReport::counts`
- `ApplyReport::counts`
- `is_video_file`
- `extract_id_from_filename`

## NFO 生成

NFO 生成由 `generate_nfo` 负责，输入数据库元数据和已存在或已成功生成的图片引用集合，输出 XML 字符串。标题字段使用 `番号ID - 标题`，演员名保留数据库原始写法，genre/tag 双写，`studio` 使用 maker 优先否则 label，`runtime` 使用 `{duration}分鍾` 风格，文本字段做 XML 转义。

当前 `--apply` 会先下载/覆盖 basename 图片，再生成 NFO，且 NFO 只引用成功下载的图片；图片失败只记录警告，不阻止 NFO。NFO 成功后才移动视频。移动优先使用 rename；rename 失败时复制、校验大小一致后删除源文件。复制成功但删除源文件失败时入库仍算成功，并报告源文件删除失败。

## 报告边界

入库整理报告由结构化结果驱动，而不是只依赖终端字符串：

- dry-run 使用 `OrganizeDryRunReport` 和 `OrganizeDryRunReport::counts`。
- apply 使用 `ApplyReport` 和 `ApplyReport::counts`。

报告分类覆盖：将入库/已入库、已存在跳过、批内冲突、目标同名冲突、缺少元数据、缺少演员信息、缺少发行日期、标题为空、NFO 失败、图片下载警告、路径清洗/截断警告、未识别视频和源文件删除失败。后续 JSON 报告应优先复用这些结构化结果，而不是解析人类可读输出。

## SMB 路径处理

输入可以是本地路径，也可以是 `smb://...` URL。

```text
smb://host/share/path
→ \\host\share\path
```

如果 SMB URL 包含用户名，则启动扫描前会尝试通过 Windows `net use` 进行认证。

相关函数：

- `resolve_scan_path`
- `convert_smb_url_to_unc`
- `authenticate_smb_if_needed`

## 修改入口

- 修改 CLI 参数或扫描模式选择：看 `src/main.rs` 的 `Args` 和 `main`。
- 修改统计扫描输出：看 `run_stats_scan`。
- 修改重复候选规则：优先看 `src/lib.rs` 的番号 ID 与分片视频规则，再看 `run_duplicate_scan` 的聚合逻辑。
- 修改番号前缀规则：看 `extract_prefix_from_id` 和 `run_prefix_scan`。
- 修改 SMB URL/UNC 路径行为：看 `convert_smb_url_to_unc`、`resolve_scan_path`、`authenticate_smb_if_needed`。

## 测试位置

核心领域规则的单元测试集中在 `src/lib.rs`。新增或修改以下规则时，应优先补充这里的测试：

- 视频文件/作品辅助媒体识别。
- 番号 ID 提取。
- 番号前缀提取。
- 分片视频识别。
- SMB URL 到 UNC 路径转换。
