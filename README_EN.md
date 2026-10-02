# CyTracer · Cyrene's Video Motion Analyzer

**简体中文** | [English](README_EN.md)

[![Release](https://github.com/Cyrene2008/CyTracer/actions/workflows/release.yml/badge.svg)](https://github.com/Cyrene2008/CyTracer/actions/workflows/release.yml)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows-0078D4.svg)](#requirements)
[![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB.svg)](https://tauri.app/)
[![Vue](https://img.shields.io/badge/Vue-3-42B883.svg)](https://vuejs.org/)

CyTracer is a fully offline Windows desktop tool for local video analysis. Import one or more recordings and it quickly scans the whole clip, automatically marking time ranges with **obvious motion**, presented on an editing-software-style timeline: player + motion intensity curve + editable markers.

> [!CAUTION]
> Made with ❤️ by [Cyrene2008](https://github.com/Cyrene2008)
>
> Powered by [Vue Fluent Widgets](https://fluent.cyrene.hk)

## Features

- Import one or many video files (drag & drop supported) into an analysis queue.
- Fast full-clip scan: block-difference motion detection with per-frame active-block ratio, 20x+ realtime.
- Adaptive threshold based on the clip's own noise floor, so single-pixel jitter is not flagged.
- Camera motion compensation: global translation from handheld shake / panning is estimated and removed, leaving local object motion.
- Scene cuts detected as a separate event type, toggleable in the UI.
- Editing-software-style timeline: ruler, motion intensity heat curve, motion markers, scene-cut marks, playhead.
- Marker editing: jump, rename, note, category color, drag start/end, merge adjacent, add manual, delete.
- Instant re-threshold: parameter changes recompute markers from cached metrics in milliseconds, no re-decoding.
- Export markers as JSON, CSV, or SRT subtitles.
- Works out of the box: FFmpeg is bundled, no runtime installation, fully offline.
- Bilingual UI (Chinese / English), light and dark themes.

## Requirements

- Windows 10 or Windows 11, 64-bit.
- No network needed: FFmpeg ships with the installer.
- 8GB+ RAM recommended; SSD recommended for caches on long 4K clips.

Installs per-user without administrator rights. Caches and preview proxies live in the user's local app data directory and can be cleared anytime.

## Install

1. Download the latest installer from [GitHub Releases](https://github.com/Cyrene2008/CyTracer/releases).
2. Run `CyTracer_<version>_x64-setup.exe` (installer language follows your system).
3. Launch and start analyzing immediately.

## Quick Start

1. Click "Import Videos" or drop video files onto the window.
2. Adjust sensitivity, minimum duration and camera compensation in the right panel (defaults are fine).
3. Click "Analyze" and wait; cancel anytime.
4. Inspect markers on the timeline: click to jump, Space to play/pause, arrow keys to step frames.
5. Edit markers as needed: rename, note, drag edges, merge or delete.
6. Click "Export" to save markers as JSON / CSV / SRT.

## How it works

1. FFmpeg extracts low-resolution grayscale frames (8fps by default) piped into the Rust engine.
2. The engine computes block-mean differences via integral images, per-frame active-block ratio and motion bounding box.
3. Row/column projection cross-correlation estimates global translation to compensate camera motion; a residual median subtraction is the fallback.
4. The noise floor is estimated from the clip's quietest segments (percentile + MAD); hysteresis and merge/duration rules extract events.
5. Scene cuts are detected by grayscale histogram L1 distance as a separate event type.
6. Per-frame metrics are cached as `.cymc` files so parameter changes re-threshold instantly.

Full algorithm notes live in the project's planning documents (development material, not shipped).

## Data & Directories

| Content | Default location |
| --- | --- |
| Application, FFmpeg (bundled decoder) | Install directory |
| Settings, markers, analysis cache, preview proxies | Current user app data dir `hk.cyrene.cytracer` |

## Build from Source

Requires Windows + [bun](https://bun.sh/) 1.3+, Rust stable, and WebView2 Runtime (preinstalled on Win11).

```bash
bun install            # install frontend deps
bun run fetch:ffmpeg   # prepare ffmpeg/ffprobe (reuse local copies or download BtbN GPL build)
bun run tauri dev      # run in development
bun run build:app      # build the NSIS installer
```

## Tech Stack

- Frontend: Vue 3 + Vite + vue-fluent-widgets + vue-router
- Backend: Tauri 2 + Rust workspace (cytracer-core / cytracer-analyze / cytracer-project / cytracer-cli)
- Decoding: bundled FFmpeg (BtbN GPL static build, distributed as sidecars)

## License

GPL-3.0-or-later. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for third-party components.
