Status: done

# 新增 organize dry-run 骨架与配置解析

## Parent

.scratch/organize-jav-media/PRD.md

## What to build

新增 `organize` 子命令的最小可运行骨架。用户可以通过完整 CLI 参数或完整本地配置启动入库整理 dry-run。此 slice 不需要实际扫描入库，也不写文件；目标是建立安全的命令入口、配置解析规则和 dry-run 默认语义。

配置来源规则必须完整实现：`source`、`target`、`database` 要么全部来自 CLI，要么全部来自配置；完整 CLI 优先；部分 CLI 参数报错；不做字段级合并。配置路径支持 `~` 展开，不支持环境变量展开。`--exclude` 可重复传入，并与配置中的 exclude 追加合并。

## Acceptance criteria

- [x] `jav-fs organize` 子命令存在，默认 dry-run。
- [x] 支持 `--source`、`--target`、`--database`、`--apply`、`--fail-fast`、`--exclude` 参数。
- [x] 完整 CLI 参数可以在没有配置文件时运行 dry-run。
- [x] 完整配置文件可以在没有 CLI 三路径参数时运行 dry-run。
- [x] 完整 CLI 参数优先于完整配置。
- [x] 只传部分 CLI 路径参数时报错，即使配置完整也不合并。
- [x] 配置不完整时报错，且不能靠部分 CLI 补齐。
- [x] 配置路径支持 `~` 展开，不展开 `$VAR` 或 `${VAR}`。
- [x] `jav-fs.toml` 被 git 忽略，示例配置存在且与解析规则一致。
- [x] dry-run 骨架不创建目录、不写文件、不下载图片、不移动视频。
- [x] 配置解析有单元测试覆盖成功、优先级和错误场景。

## Blocked by

- .scratch/organize-jav-media/issues/01-extend-video-file-recognition.md
