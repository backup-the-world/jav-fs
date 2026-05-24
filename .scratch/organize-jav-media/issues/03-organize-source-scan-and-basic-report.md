Status: done

# 实现入库源扫描与基础 dry-run 报告

## Parent

.scratch/organize-jav-media/PRD.md

## What to build

让 `jav-fs organize` 的 dry-run 真正扫描 incoming 源目录，产出基础报告。扫描应递归进入非隐藏子目录，跳过隐藏文件和隐藏目录，应用内置默认排除规则以及配置/CLI 追加的 exclude 规则。

此 slice 只需要完成候选视频发现、番号 ID 提取、未识别视频分类，以及“提取到番号 ID 但数据库无记录”的缺少元数据分类。数据库访问可以先只查询 `videos.product_id` 是否存在，不要求完整元数据或目标路径规划。

## Acceptance criteria

- [x] dry-run 递归扫描 source 下的非隐藏目录。
- [x] 隐藏文件和隐藏目录被跳过。
- [x] 默认排除规则跳过 `@eaDir`、`tmp`、`temp`、`incomplete` 等目录。
- [x] 配置和 CLI 中的 exclude 规则会追加生效。
- [x] 只把统一视频扩展名范围内的文件作为候选视频。
- [x] 候选视频沿用现有番号 ID 提取规则。
- [x] 无法提取番号 ID 的视频报告为未识别视频。
- [x] 能提取番号 ID 但数据库无记录的视频报告为缺少元数据。
- [x] dry-run 报告包含候选数量、未识别视频和缺少元数据列表。
- [x] dry-run 不写文件、不创建目录、不下载图片、不移动视频。
- [x] 源扫描和基础报告有测试覆盖，包括隐藏路径、exclude、未识别视频和缺少元数据。

## Blocked by

- .scratch/organize-jav-media/issues/02-organize-dry-run-cli-and-config.md
