# 第三方组件与许可证声明 / Third-Party Notices

CyTracer 使用了以下第三方组件。分发本软件时请一并保留本声明。

CyTracer bundles the following third-party components. Keep this notice when redistributing.

## 组件列表 / Components

### FFmpeg（内置解码器 / Bundled decoder）

- 用途 / Purpose：视频探测、解码抽帧、预览代理转码 / media probing, frame extraction, preview proxy transcoding
- 来源 / Source：https://github.com/BtbN/FFmpeg-Builds （`ffmpeg-master-latest-win64-gpl`）；本地开发副本可能来自 https://www.gyan.dev/ffmpeg/builds/ （essentials，同为 GPL 构建）
- 许可证 / License：GPL（构建启用的组件决定，详见 FFmpeg 官方说明）/ GPL (as enabled by the build; see FFmpeg licensing)
- 说明：本项目以 GPL-3.0-or-later 发布，FFmpeg 以独立可执行文件（sidecar）形式随安装包分发。
  CyTracer is released under GPL-3.0-or-later; FFmpeg is distributed as separate executables (sidecars).

### Vue Fluent Widgets

- 用途 / Purpose：Fluent Design（WinUI3）Vue 组件库 / Fluent Design component library for Vue
- 来源 / Source：https://github.com/Cyrene2008/VueFluentWidgets
- 许可证 / License：MIT
- 版权 / Copyright：© 2025–2026 Cyrene2008

### MiSans 字体 / MiSans Font

- 用途 / Purpose：界面字体 / UI typeface
- 来源 / Source：小米公司 / Xiaomi Inc.
- 许可证 / License：MiSans 字体许可（免费商用）
- 版权 / Copyright：© Xiaomi Inc.

### Rust / JavaScript 依赖 / Rust & JavaScript Dependencies

- Tauri、Vue 3、Vite、serde、rayon、rustfft 等，均以各自许可证分发。
- Tauri, Vue 3, Vite, serde, rayon, rustfft and others are distributed under their respective licenses.
