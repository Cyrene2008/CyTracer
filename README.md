# CyTracer · Cyreneの视频分析器

**Cyrene's Video Motion Analyzer**

[English](README_EN.md) | **简体中文**

[![Release](https://github.com/Cyrene2008/CyTracer/actions/workflows/release.yml/badge.svg)](https://github.com/Cyrene2008/CyTracer/actions/workflows/release.yml)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows-0078D4.svg)](#系统要求)
[![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB.svg)](https://tauri.app/)
[![Vue](https://img.shields.io/badge/Vue-3-42B883.svg)](https://vuejs.org/)

CyTracer 是一款纯离线的本地视频分析桌面工具。导入一个或多个录像后，程序会快速扫描全片，自动标记出画面中存在**明显移动**的时间区间，并用剪辑软件风格的时间轴呈现：播放器 + 运动强度曲线 + 可编辑的标记。

> [!CAUTION]
> Made with ❤️ by [Cyrene2008](https://github.com/Cyrene2008)
>
> Powered by [Vue Fluent Widgets](https://fluent.cyrene.hk)

## 主要功能

- 导入 1 个或多个视频文件（支持拖拽），建立批量分析队列。
- 快速全片扫描：块差异运动检测，逐帧计算活跃块比例，速度可达 20 倍以上实时。
- 自适应阈值：根据视频噪声底自动设定触发门槛，避免 1 像素级抖动误标。
- 相机运动补偿：默认估算并补偿手持抖动 / 摇镜的全局位移，只标局部物体移动。
- 镜头切换单独识别为独立事件类型，可开关显示。
- 剪辑软件风格时间轴：时间标尺、运动强度热力曲线、移动标记块、镜头切换标记、播放头。
- 标记编辑：跳转、重命名、备注、类别颜色、拖拽调整起止、合并相邻、手动补标、删除。
- 参数实时重算：修改灵敏度 / 最短持续等参数后，基于缓存指标秒级刷新标记，无需重新解码。
- 导出标记为 JSON、CSV、SRT 字幕。
- 开箱即用：内置 FFmpeg 解码器，无需安装任何运行环境，断网可用。
- 中英双语界面，支持浅色 / 深色主题。

## 系统要求

- Windows 10 或 Windows 11，64 位。
- 无需网络：FFmpeg 随安装包内置，安装即用。
- 建议 8GB 以上内存；4K 长视频建议使用 SSD 存放缓存。

程序采用当前用户安装模式，不要求管理员权限。分析缓存与预览代理位于用户本地应用数据目录，可随时清理。

## 安装

1. 从 [GitHub Releases](https://github.com/Cyrene2008/CyTracer/releases) 下载最新的 Windows 安装包。
2. 运行 `CyTracer_<版本>_x64-setup.exe`（安装器语言自动跟随系统语言）。
3. 启动后直接进入主界面，无需等待任何运行环境准备。

## 快速使用

1. 点击「导入视频」或直接拖入视频文件。
2. 在右侧参数面板调整灵敏度、最短持续、相机运动补偿等（可用默认值）。
3. 点击「开始分析」，等待进度完成；可随时取消。
4. 在时间轴上查看运动标记，点击标记跳转到对应片段，空格播放 / 暂停，方向键逐帧。
5. 按需编辑标记：重命名、备注、拖动起止点、合并或删除。
6. 点击「导出」选择 JSON / CSV / SRT 输出标记。

## 分析原理（简述）

1. FFmpeg 将视频抽帧为低分辨率灰度图（默认 8fps）并通过管道送入 Rust 引擎。
2. 引擎以积分图计算分块均值差，统计每帧活跃块比例，并记录运动区域包围盒。
3. 行 / 列投影互相关估算全局位移，补偿相机运动；残差再做中位差兜底。
4. 基于视频安静段的分位数与 MAD 估计噪声底，双阈值滞回提取事件，合并、去抖。
5. 灰度高直方图 L1 距离检测镜头切换，作为独立事件类型。
6. 逐帧指标缓存为 `.cymc` 文件，改参数时读取缓存秒级重算标记。

详细算法说明见项目规划文档（开发资料，不随仓库分发）。

## 数据与目录

| 内容 | 默认位置 |
| --- | --- |
| 应用程序、FFmpeg（内置解码器） | 软件安装目录 |
| 设置、标记、分析缓存、预览代理 | 当前用户应用数据目录 `hk.cyrene.cytracer` |

## 从源码构建

需要 Windows + [bun](https://bun.sh/) 1.3+、Rust stable、WebView2 Runtime（Win10 需安装，Win11 自带）。

```bash
bun install            # 安装前端依赖
bun run fetch:ffmpeg   # 准备 ffmpeg/ffprobe（优先复用本地，否则下载 BtbN GPL 构建）
bun run tauri dev      # 开发运行
bun run build:app      # 构建 NSIS 安装包
```

## 技术栈

- 前端：Vue 3 + Vite + vue-fluent-widgets + vue-router
- 后端：Tauri 2 + Rust 工作区（cytracer-core / cytracer-analyze / cytracer-project / cytracer-cli）
- 解码：内置 FFmpeg（BtbN GPL 静态构建，sidecar 方式分发）

## 许可证

GPL-3.0-or-later。第三方组件声明见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
