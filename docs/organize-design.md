# 入库整理设计

本文记录 `jav-fs organize` 的设计收束，用于把 incoming 区域中的视频文件整理到正式 JAV 媒体库。

## 目标

将 `/Volumes/jav/incoming/incoming-hd/` 中的视频文件按数据库元数据整理到 `/Volumes/jav/media/hd/` 的既有目录结构中。数据库默认来自 `~/dev/jav/jav-hub/data/jav-data.db`，但路径不硬编码，通过配置或 CLI 提供。

整理以 **JAV 作品** 为组织核心，同时保留每个视频文件和分片视频的独立性。

## 命令行

使用子命令：

```sh
jav-fs organize

jav-fs organize \
  --source /Volumes/jav/incoming/incoming-hd \
  --target /Volumes/jav/media/hd \
  --database ~/dev/jav/jav-hub/data/jav-data.db

jav-fs organize \
  --source /Volumes/jav/incoming/incoming-hd \
  --target /Volumes/jav/media/hd \
  --database ~/dev/jav/jav-hub/data/jav-data.db \
  --apply
```

默认 dry-run；只有传入 `--apply` 才写文件、下载图片或移动视频。

`--fail-fast`：执行模式下遇到首个作品失败时停止；默认继续处理后续作品。

`--exclude <GLOB>`：可重复传入，与配置中的排除规则追加合并。

## 配置文件

配置文件为当前工作目录下的 `jav-fs.toml`，不提交到仓库；仓库提供 `jav-fs.example.toml`。

```toml
[organize]
source = "/Volumes/jav/incoming/incoming-hd"
target = "/Volumes/jav/media/hd"
database = "~/dev/jav/jav-hub/data/jav-data.db"
exclude = ["**/sample/**"]
```

路径支持 `~` 展开，不支持环境变量展开。

`source`、`target`、`database` 的来源必须独立完整：

- 完整 CLI 三项优先于配置。
- CLI 三项完全不传时，使用完整配置。
- 只传部分 CLI 参数时报错，即使配置完整也不做字段级合并。
- 配置不完整不能靠 CLI 补齐；CLI 不完整也不能靠配置补齐。

## 扫描范围

- 递归扫描 source 下所有非隐藏子目录。
- 跳过所有隐藏文件和隐藏目录。
- 内置默认排除目录，并支持配置/CLI 追加排除规则。

默认排除建议：

```text
**/@eaDir/**
**/tmp/**
**/temp/**
**/incomplete/**
**/.*/**
```

视频扩展名统一扩展为：

```text
.mp4 .mkv .wmv .avi .mov .m4v .ts
```

该范围应同时用于现有统计扫描、重复扫描、前缀扫描和入库整理。

## 番号识别与数据库

入库整理沿用当前番号 ID 提取规则：从文件名提取一个番号 ID，然后查数据库。

- 查到数据库记录：继续入库计划。
- 能提取 ID 但数据库无记录：跳过，报告“缺少元数据”。
- 无法提取 ID：跳过，报告“未识别视频”。
- 不处理多候选/合集歧义；第一版假设 incoming 文件名只有一个有效番号。

数据库需要提供：

- `videos.product_id`
- `videos.title`
- `videos.release_date`
- `videos.duration`
- `videos.description`
- `videos.series`
- `videos.label`
- `videos.maker`
- `videos.cover_image`
- `videos.cover_image_landscape`
- `videos.cover_image_portrait`
- `videos.preview_images`
- `directors.name`
- `actresses.name`
- `genres.name`

如果缺少演员信息、发行日期或标题为空，则跳过，不入库。

标题中的路径非法字符会安全清洗；作品目录名过长时截断标题，但必须保留 `[年份] 番号ID - ` 前缀。清洗或截断写入报告的“路径清洗/截断警告”。

## 目标目录结构

沿用现有媒体库结构：

```text
/Volumes/jav/media/hd/
  #演员名,演员名/
    [年份] 番号ID - 标题/
      番号ID.mp4
      番号ID.nfo
      番号ID-poster.jpg
      番号ID-thumb.jpg
      番号ID-fanart.jpg
      extrafanart/
        extrafanart-1.jpg
        extrafanart-2.jpg
```

演员目录：

- 目录名前缀为 `#`。
- 演员名来自数据库，但目录名去掉半角/全角空格。
- 多演员用逗号连接。
- 若已有同演员集合目录，则复用现有目录名。
- 没有现有目录时，按 `actresses.id` 升序生成。

作品目录：

```text
[年份] 番号ID - 标题
```

年份来自 `release_date`。番号 ID 统一大写。标题使用清洗/截断后的数据库标题。

## 视频命名

目标视频文件名规范化：

- 去下载站前缀。
- 番号 ID 统一大写。
- 普通单文件：`ABF-323.mp4`。
- 分片视频：`DSVR01794-CD1.mp4`、`DSVR01794-CD2.mp4`。
- 不保留 `_8k`、站点名等来源噪音。

同一批次内按番号 ID 聚合：

- 能识别为不同分片的视频一起入库。
- 同番号但无法区分为不同分片时，整组跳过并报告“批内冲突”。

目标作品目录已存在时：

- 允许追加同作品下未冲突的新视频/新分片。
- 同名目标视频已存在时，跳过该视频并报告“目标同名冲突”。
- 被跳过的视频不生成或覆盖 NFO、图片。

## NFO

每个视频文件生成一个同 basename 的 NFO：

```text
ABF-323.mp4      -> ABF-323.nfo
ABP-312-CD1.mp4  -> ABP-312-CD1.nfo
```

NFO 使用现有 XML 风格，但只填数据库有的字段，不伪造评分或网站等缺失信息。

规则：

- `title` / `originaltitle` / `sorttitle`：`番号ID - 标题`，不包含年份或分片号。
- 演员名在 NFO 中保留数据库原始写法，不去空格。
- 每个数据库 genre 同时写 `<tag>` 和 `<genre>`。
- `studio = maker 优先，否则 label`。
- `maker = maker`。
- `label = label`。
- `set = series`。
- `runtime = {duration}分鍾`。
- `description` 同时写入 `outline` 和 `plot`。
- 只写实际存在或本轮成功生成的图片引用。

NFO 生成失败时，不移动视频；该作品或视频入库失败。

## 图片

需要尝试按数据库 URL 下载封面和预览图。图片失败不阻止视频入库，只报告“图片下载警告”。

basename 图片命名：

```text
ABF-323-poster.jpg
ABF-323-thumb.jpg
ABF-323-fanart.jpg
```

字段映射：

- `cover_image_portrait` -> `basename-poster.jpg`
- 若无 `cover_image_portrait`，`cover_image` -> `basename-poster.jpg`
- `cover_image_landscape` -> `basename-fanart.jpg`
- 若无 `cover_image_landscape`，不生成 fanart
- `cover_image` -> `basename-thumb.jpg`
- `preview_images` -> `extrafanart/extrafanart-N.jpg`

下载图片时覆盖已有目标图片文件，以数据库当前 URL 为准。

追加新分片时：

- 为新分片生成自己的 NFO 和 basename 图片。
- `extrafanart/` 若已存在则不重复下载；缺失时可尝试下载。

## 执行顺序

每个作品或视频采用“先准备、后移动”：

1. 查数据库、计算目标路径、检查冲突。
2. 创建目标目录。
3. 下载/覆盖图片，记录成功和失败。
4. 生成 NFO，只引用成功下载或已存在的图片。
5. 移动视频。
6. 图片失败只作为警告。

如果图片下载成功，但 NFO 生成失败导致视频不移动：

- 清理本轮新增文件。
- 新建作品目录若最终没有成功入库任何视频，则删除本轮创建的空目录。
- 不恢复已覆盖图片；该边界写入报告。

## 移动语义

整理成功后源视频应离开 incoming。

移动优先使用 `rename`。如果跨文件系统导致 rename 失败，则复制到目标、校验大小一致后删除源文件。

如果复制成功且校验通过，但删除源文件失败：

- 入库仍算成功。
- 报告“源文件删除失败”，需要人工清理 incoming。

## dry-run 与 apply

Dry-run：

- 默认模式。
- 绝不写文件、不创建目录、不移动视频、不下载图片。
- 不联网探测图片 URL。
- 只基于数据库和文件系统状态生成计划。
- 报告将创建目录、移动视频、生成 NFO、下载/覆盖图片、跳过原因和警告。

Apply：

- 只有传 `--apply` 才执行。
- 不二次交互确认；dry-run 是确认机制。
- 默认遇到失败继续处理后续作品。
- `--fail-fast` 可在首个作品失败时停止。

## 报告分类

Dry-run 和 apply 输出同一套分类；apply 显示实际结果。

- 将入库 / 已入库
- 已存在跳过
- 批内冲突
- 目标同名冲突
- 缺少元数据
- 缺少演员信息
- 缺少发行日期
- 标题为空
- NFO 失败
- 图片下载警告
- 路径清洗/截断警告
- 未识别视频
- 源文件删除失败

报告打印到终端。后续可增加 `--report report.json` 输出机器可读报告。
