Status: done

# 完善报告、CLI 文档与架构/识别规则文档

## Parent

.scratch/organize-jav-media/PRD.md

## What to build

打磨入库整理的用户可见报告和项目文档。dry-run 和 apply 应使用同一套报告分类；dry-run 展示计划，apply 展示实际结果。文档需要说明 `organize` 子命令、配置文件、dry-run/apply、安全边界、报告分类、视频扩展名变更，以及入库整理在整体架构中的模块边界。

此 slice 不实现 JSON 报告；仅确保终端报告稳定、可读、覆盖所有分类，并为未来机器可读报告保留结构化结果基础。

## Acceptance criteria

- [x] dry-run 和 apply 使用同一套报告分类。
- [x] 报告包含将入库/已入库、已存在跳过、批内冲突、目标同名冲突、缺少元数据、缺少演员信息、缺少发行日期、标题为空、NFO 失败、图片下载警告、路径清洗/截断警告、未识别视频、源文件删除失败。
- [x] dry-run 明确标识不会写文件、不会下载、不会移动。
- [x] apply 明确展示实际入库、跳过和警告结果。
- [x] 报告由结构化结果驱动，测试不只依赖脆弱的人类可读字符串。
- [x] CLI 文档说明 `organize` 用法、配置文件、完整 CLI/完整配置规则、`--apply`、`--fail-fast` 和 `--exclude`。
- [x] 架构文档说明入库整理主要模块边界，强调复杂规则不堆在 CLI 编排层。
- [x] 识别规则文档同步说明扩展后的视频文件定义和整理命名规则。
- [x] 示例配置与文档保持一致。
- [x] 回归测试确认现有统计扫描、重复扫描、前缀扫描仍可运行。

## Blocked by

- .scratch/organize-jav-media/issues/07-apply-organize-move-and-cleanup.md
