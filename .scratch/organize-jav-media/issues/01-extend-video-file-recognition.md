Status: done

# 扩展视频文件识别并保持现有扫描一致

## Parent

.scratch/organize-jav-media/PRD.md

## What to build

扩展 JAV 文件管理中的视频文件定义，让统计扫描、重复扫描、前缀扫描和后续入库整理都使用同一组视频扩展名：`.mp4/.mkv/.wmv/.avi/.mov/.m4v/.ts`。

这个 slice 应保持现有扫描行为兼容，只改变哪些扩展名被视为视频文件，并同步测试和文档说明。

## Acceptance criteria

- [x] `.avi`、`.mov`、`.m4v`、`.ts` 被识别为视频文件。
- [x] 现有 `.mp4`、`.mkv`、`.wmv` 识别行为保持不变，包括大小写扩展名。
- [x] 非视频扩展名仍不会被识别为视频文件。
- [x] 统计扫描、重复扫描、前缀扫描使用同一视频文件定义。
- [x] 相关单元测试覆盖新增扩展名和既有回归场景。
- [x] 识别规则文档更新，说明新的视频扩展名范围。

## Blocked by

None - can start immediately
