Status: done

# 下载作品辅助媒体并让 NFO 只引用成功图片

## Parent

.scratch/organize-jav-media/PRD.md

## What to build

为入库整理增加作品辅助媒体下载。apply 时根据数据库 URL 下载 poster、thumb、fanart 和 `extrafanart/` 预览图；dry-run 不联网，只报告将下载或覆盖哪些图片。图片下载失败不阻止视频入库，只报告图片下载警告。

该 slice 需要调整准备顺序：先下载或确认图片，再生成 NFO，让 NFO 只引用实际存在或本轮成功下载的图片。

## Acceptance criteria

- [x] dry-run 不联网、不下载图片，只报告图片下载/覆盖计划。
- [x] apply 根据 `cover_image_portrait` 优先生成 `basename-poster.jpg`。
- [x] 无 `cover_image_portrait` 时，apply 使用 `cover_image` 生成 `basename-poster.jpg`。
- [x] apply 使用 `cover_image` 生成 `basename-thumb.jpg`。
- [x] apply 使用 `cover_image_landscape` 生成 `basename-fanart.jpg`；无 landscape 时不生成 fanart。
- [x] apply 将 `preview_images` 下载到作品级 `extrafanart/extrafanart-N.jpg`。
- [x] 图片下载覆盖本次成功入库视频对应的已有目标图片。
- [x] 追加新分片时，为新分片生成 basename 图片。
- [x] 作品级 `extrafanart/` 已存在时不重复下载；缺失时可尝试下载。
- [x] 图片下载失败不阻止 NFO 生成和视频入库，只报告图片下载警告。
- [x] NFO 只引用实际存在或本轮成功下载的图片。
- [x] 图片下载模块可通过测试替身验证成功、失败、覆盖和 URL 映射，不依赖真实外网。

## Blocked by

- .scratch/organize-jav-media/issues/05-generate-nfo-for-organize.md
