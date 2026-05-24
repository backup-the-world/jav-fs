Status: done

# 执行视频移动、跨卷回退与失败清理

## Parent

.scratch/organize-jav-media/PRD.md

## What to build

完成 `jav-fs organize --apply` 的真实入库执行路径。对每个将入库的视频，执行顺序应为：检查计划、创建目标目录、下载/确认图片、生成 NFO、最后移动视频。成功入库后源视频离开 incoming。默认遇到单个失败继续处理后续项目；`--fail-fast` 在首个失败时停止。

移动优先使用 rename；跨文件系统失败时复制到目标、校验大小一致后删除源文件。复制成功且校验通过但删除源文件失败时，入库仍算成功，并报告源文件删除失败。

## Acceptance criteria

- [x] `--apply` 会创建必要目标目录。
- [x] `--apply` 按“图片准备 -> NFO 生成 -> 视频移动”的顺序执行。
- [x] NFO 生成失败时不移动视频，并报告 NFO 失败。
- [x] NFO 失败后清理本轮新增文件。
- [x] 新建作品目录若最终没有成功入库任何视频，会清理本轮创建的空目录。
- [x] 目标同名视频冲突时不覆盖视频，不生成或覆盖该视频对应 NFO/图片。
- [x] 成功入库后源视频离开 incoming，目标视频出现在规划位置。
- [x] 移动优先使用 rename。
- [x] rename 因跨文件系统失败时，复制目标、校验大小一致后删除源文件。
- [x] 复制大小校验失败时不删除源文件，并报告失败。
- [x] 复制成功但删除源文件失败时，入库算成功，并报告源文件删除失败。
- [x] 默认遇到失败继续处理后续视频或作品。
- [x] `--fail-fast` 在首个失败后停止后续执行。
- [x] apply 端到端测试使用临时目录和临时数据库覆盖 happy path、目标冲突、NFO 失败、删除源失败和 fail-fast。

## Blocked by

- .scratch/organize-jav-media/issues/06-download-organize-artwork.md
