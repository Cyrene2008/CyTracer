<script setup>
import { computed } from 'vue'
import { backendState } from '../../stores/backend'
import logo from '../../assets/logo.png'

const version = computed(() => backendState.version || '26.0.1')

async function minimize () {
  const { getCurrentWindow } = await import('@tauri-apps/api/window')
  await getCurrentWindow().minimize()
}

async function toggleMaximize () {
  const { getCurrentWindow } = await import('@tauri-apps/api/window')
  const win = getCurrentWindow()
  if (await win.isMaximized()) {
    await win.unmaximize()
  } else {
    await win.maximize()
  }
}

async function close () {
  const { getCurrentWindow } = await import('@tauri-apps/api/window')
  await getCurrentWindow().close()
}
</script>

<template>
  <FluentTitleBar
    class="titlebar"
    :draggable="false"
    :show-window-controls="true"
    @minimize="minimize"
    @maximize="toggleMaximize"
    @close="close"
  >
    <div class="titlebar-content">
      <span class="brand">
        <img class="brand-logo" :src="logo" alt="CyTracer" draggable="false" />
        <span class="brand-text">Cyreneの视频分析器</span>
        <span class="brand-version">v{{ version }}</span>
      </span>
    </div>
  </FluentTitleBar>
</template>

<style scoped>
.titlebar {
  --titlebar-height: 40px;
}
.titlebar-content {
  display: flex;
  align-items: center;
  height: 100%;
  width: 100%;
  padding: 0 4px 0 12px;
  -webkit-app-region: drag;
}
.brand {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  user-select: none;
}
.brand-logo {
  width: 20px;
  height: 20px;
  border-radius: 5px;
  -webkit-user-drag: none;
}
.brand-text {
  font-size: 13px;
  color: var(--text-primary, #222);
}
.brand-version {
  font-size: 11px;
  color: var(--text-muted, #888);
}
</style>
