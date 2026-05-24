# CLI 使用说明

`jav-fs` 用于扫描 JAV 媒体库，当前支持统计扫描、重复扫描、前缀扫描和入库整理 dry-run 骨架。

## 基本用法

```sh
jav-fs [URL_OR_PATH] [OPTIONS]
```

如果不传路径，默认扫描当前目录：

```sh
jav-fs
```

扫描本地路径：

```sh
jav-fs /path/to/library
```

扫描 SMB URL：

```sh
jav-fs smb://nas/video
```

指定线程数：

```sh
jav-fs /path/to/library --threads 4
```

## 统计扫描

默认模式是统计扫描：

```sh
jav-fs /path/to/library
```

统计扫描会输出：

- 文件总数量。
- 视频文件数量和总大小。
- 作品辅助媒体数量和总大小。

视频文件统一识别为 `.mp4`、`.mkv`、`.wmv`、`.avi`、`.mov`、`.m4v`、`.ts`，扩展名大小写不敏感；统计扫描、重复扫描、前缀扫描和入库整理使用同一识别规则。

## 重复扫描

使用 `--show-duplicate` 执行重复扫描：

```sh
jav-fs /path/to/library --show-duplicate
```

重复扫描会：

- 只处理视频文件。
- 从文件名提取番号 ID。
- 按番号 ID 聚合同一 JAV 作品。
- 排除不同分片视频。
- 输出重复候选路径。

注意：重复候选不等于确认可删除的重复文件。删除前仍需人工确认清晰度、字幕、版本或内容差异。

## 前缀扫描

使用 `--show-prefix` 执行前缀扫描：

```sh
jav-fs /path/to/library --show-prefix
```

前缀扫描会输出 JAV 媒体库中出现过的唯一番号前缀。

## 入库整理 dry-run

使用 `organize` 子命令执行入库整理。该命令默认 dry-run；会递归扫描 source、读取数据库元数据、规划目标路径，并报告候选数量、将入库、未识别视频、缺少元数据、批内冲突、目标同名冲突和路径警告。dry-run 明确不会创建目录、写文件、下载图片或移动视频。

完整 CLI 参数可以在没有配置文件时运行：

```sh
jav-fs organize --source /incoming --target /media --database ~/data/jav-data.db
```

也可以使用当前工作目录下的 `jav-fs.toml`：

```toml
[organize]
source = "/Volumes/jav/incoming/incoming-hd"
target = "/Volumes/jav/media/hd"
database = "~/dev/jav/jav-hub/data/jav-data.db"
exclude = ["**/sample/**"]
```

配置规则：

- `source`、`target`、`database` 必须全部来自 CLI，或全部来自配置。
- 完整 CLI 路径参数优先于完整配置。
- 只传部分 CLI 路径参数会报错，即使配置完整也不会字段级合并。
- 配置缺少任一必需路径会报错，不能靠部分 CLI 补齐。
- 配置路径支持 `~` 展开，不展开 `$VAR` 或 `${VAR}`。
- `--exclude` 可重复传入，并追加到配置中的 `exclude`。
- `jav-fs.toml` 是本地配置，已被 Git 忽略；提交示例请使用 `jav-fs.example.toml`。

可用参数：

```sh
jav-fs organize \
  --source /incoming \
  --target /media \
  --database ~/data/jav-data.db \
  --exclude tmp \
  --exclude incomplete
```

`--apply` 会按规划创建目标目录、下载/覆盖 basename 图片、写入同 basename 的 `.nfo` 文件，最后移动视频。图片失败只作为警告，NFO 只引用成功下载的图片。移动优先使用 rename；rename 失败时复制到目标、校验大小一致后删除源文件。`--fail-fast` 会在首个 NFO 或移动失败后停止。

终端报告分类包括：将入库/已入库、已存在跳过、批内冲突、目标同名冲突、缺少元数据、缺少演员信息、缺少发行日期、标题为空、NFO 失败、图片下载警告、路径清洗/截断警告、未识别视频和源文件删除失败。报告由结构化结果计数驱动，后续可以扩展机器可读 JSON 输出。

## SMB URL

SMB URL 会被转换为 UNC 路径：

```text
smb://host/share/path → \\host\share\path
```

如果 URL 中包含用户名和密码，程序会尝试执行 SMB 认证：

```sh
jav-fs smb://user:pass@host/share
```

当前认证方式依赖 Windows `net use` 命令。

## 参数

| 参数 | 含义 |
|---|---|
| `URL_OR_PATH` | 扫描源：要扫描的本地路径或 SMB URL；默认是当前目录。 |
| `--threads <N>` / `-t <N>` | 指定并发扫描线程数。 |
| `--show-prefix` | 执行前缀扫描。 |
| `--show-duplicate` | 执行重复扫描。 |
| `organize` | 入库整理子命令，默认 dry-run。 |
| `organize --source <PATH>` | 指定入库源路径，必须与 `--target`、`--database` 一起传入。 |
| `organize --target <PATH>` | 指定正式 JAV 媒体库目标路径，必须与 `--source`、`--database` 一起传入。 |
| `organize --database <PATH>` | 指定本地 SQLite 元数据数据库路径，必须与 `--source`、`--target` 一起传入。 |
| `organize --apply` | 保留的实际执行开关；未传时为 dry-run。 |
| `organize --fail-fast` | 保留的失败即停开关。 |
| `organize --exclude <PATTERN>` | 追加排除规则，可重复传入。 |

不要同时传入 `--show-prefix` 和 `--show-duplicate`。这两个参数表达不同扫描意图，目标行为应是互斥并提示用户只能选择一种模式；当前代码仍会优先执行前缀扫描，后续应修正为报错。
