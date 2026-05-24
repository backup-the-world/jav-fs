# CLI 使用说明

`jav-fs` 用于扫描 JAV 媒体库，当前支持统计扫描、重复扫描和前缀扫描。

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

不要同时传入 `--show-prefix` 和 `--show-duplicate`。这两个参数表达不同扫描意图，目标行为应是互斥并提示用户只能选择一种模式；当前代码仍会优先执行前缀扫描，后续应修正为报错。
