Status: done

# 生成同 basename 的 NFO 并纳入准备流程

## Parent

.scratch/organize-jav-media/PRD.md

## What to build

为入库整理实现 NFO 生成能力。每个将入库的视频文件都应生成一个同 basename 的 XML NFO。dry-run 报告将生成哪些 NFO；apply 时写入 NFO。此 slice 可以先不实现图片下载，但 NFO 生成接口应支持传入“已存在或已成功生成的图片引用集合”，以便后续图片 slice 接入。

NFO 使用正式库现有 XML 风格，只填数据库真实存在的字段，不伪造评分或网站等缺失信息。NFO 生成失败时，该视频不应被移动，并报告 NFO 失败。

## Acceptance criteria

- [x] 每个规划入库的视频都有同 basename 的 NFO 计划。
- [x] apply 会写入同 basename 的 `.nfo` 文件。
- [x] NFO 的 `title`、`originaltitle`、`sorttitle` 使用 `番号ID - 标题`，不包含年份或分片号。
- [x] NFO 中演员名保留数据库原始写法，不去空格。
- [x] 每个数据库 genre 同时写入 `<tag>` 和 `<genre>`。
- [x] `studio` 使用 maker 优先、否则 label；`maker`、`label`、`set` 按数据库字段映射。
- [x] `runtime` 使用 `{duration}分鍾` 风格。
- [x] `description` 同时写入 `outline` 和 `plot`。
- [x] NFO 只引用传入的实际存在或成功生成的图片引用。
- [x] NFO XML 对标题和描述等文本使用安全转义或 CDATA，能处理特殊字符。
- [x] NFO 生成失败时不移动视频，并报告 NFO 失败。
- [x] dry-run 不写 NFO，只报告计划。
- [x] NFO 生成模块有纯单元测试覆盖字段映射、genre/tag、runtime、CDATA/转义和图片引用。

## Blocked by

- .scratch/organize-jav-media/issues/04-organize-target-path-planning.md
