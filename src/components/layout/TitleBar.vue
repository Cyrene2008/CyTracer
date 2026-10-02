<script setup>
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { backendState } from '../../stores/backend'

const router = useRouter()
const version = computed(() => backendState.version || '26.0.0')

const query = (e) => {
  window.open('https://github.com/Cyrene2008/CyTracer/releases', '_blank')
  e?.preventDefault?.()
}

async function minimize () {
  const { getCurrentWindow } = await import('@tauri-apps/api/window')
  await getCurrentWindow().minimize()
}

async function toggleMaximize () {
  const { getCurrentWindow } = await import('@tauri-apps/api/window')
  await getCurrentWindow().toggleMaximize()
}

async function close () {
  const { getCurrentWindow } = await import('@tauri-apps/api/window')
  await getCurrentWindow().close()
}
</script>

<template>
  <FluentTitleBar class="titlebar" :show-window-controls="true" @minimize="minimize" @maximize="toggleMaximize" @close="close">
    <div class="titlebar-content">
      <span class="brand" @dblclick="router.push('/about')">
        <span class="brand-mark">Cy</span>
        <span class="brand-text">Cyreneの视频分析器</span>
        <span class="brand-version">v{{ version }}</span>
      </span>
      <span class="spacer" />
      <button class="ghost" title="GitHub Releases" @click="query">更新</button>
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
  gap: 8px;
  height: 100%;
  width: 100%;
  padding: 0 4px 0 12px;
  -webkit-app-region: drag;
}
.brand {
  display: inline-flex;
  align-items: baseline;
  gap: 8px;
  user-select: none;
}
.brand-mark {
  font-weight: 700;
  background: var(--accent, #ea5ec1);
  color: #fff;
  border-radius: 6px;
  padding: 1px 6px;
  font-size: 12px;
}
.brand-text {
  font-size: 13px;
  color: var(--text-primary, #222);
}
.brand-version {
  font-size: 11px;
  color: var(--text-muted, #888);
}
.spacer { flex: 1; }
.ghost {
  -webkit-app-region: no-drag;
  border: none;
  background: transparent;
  color: var(--text-secondary, #555);
  font-size: 12px;
  padding: 4px 8px;
  border-radius: 6px;
  cursor: pointer;
}
.ghost:hover {
  background: var(--bg-hover, rgba(0, 0, 0, 0.06));
}
</style>
