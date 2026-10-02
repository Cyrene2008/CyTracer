<script setup>
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import MediaBinPanel from '../components/MediaBinPanel.vue'
import VideoStage from '../components/VideoStage.vue'
import TimelinePanel from '../components/timeline/TimelinePanel.vue'
import InspectorPanel from '../components/InspectorPanel.vue'
import ExportDialog from '../components/ExportDialog.vue'
import {
  analyzeAll,
  analyzeVideo,
  cancelAnalysis,
  currentVideo,
  importPaths,
  library
} from '../stores/library'
import { onDragDrop } from '../utils/api'
import { t } from '../utils/i18n'

const showExport = ref(false)
const dragActive = ref(false)
let unlisten = null

const video = computed(() => currentVideo.value)
const analyzing = computed(() =>
  library.videos.some((v) => v.status === 'analyzing') || library.queueRunning
)

const statusText = computed(() => {
  if (!video.value) return ''
  switch (video.value.status) {
    case 'analyzing': return `${t('status.analyzing')} ${Math.round((video.value.progress || 0) * 100)}%`
    case 'done': return t('status.done')
    case 'failed': return t('status.failed')
    case 'canceled': return t('status.canceled')
    case 'probing': return t('status.probing')
    default: return t('status.pending')
  }
})

function analyzeCurrent () {
  if (!video.value) return
  analyzeVideo(video.value.id)
}

async function onDropPaths (paths) {
  if (paths?.length) await importPaths(paths)
}

onMounted(async () => {
  unlisten = await onDragDrop((payload) => {
    if (!payload) return
    if (payload.type === 'enter' || payload.type === 'over') {
      dragActive.value = true
    } else if (payload.type === 'leave') {
      dragActive.value = false
    } else if (payload.type === 'drop') {
      dragActive.value = false
      onDropPaths(payload.paths)
    }
  })
})

onBeforeUnmount(() => {
  if (typeof unlisten === 'function') unlisten()
})
</script>

<template>
  <div class="page analyze-page">
    <div class="workspace-head">
      <FluentIcon icon="fluent:video-24-regular" width="16" height="16" />
      <span class="head-title" :title="video?.path">{{ video?.name || 'CyTracer' }}</span>
      <span v-if="statusText" class="head-status" :class="`s-${video?.status}`">{{ statusText }}</span>
      <span class="spacer" />
      <FluentButton
        v-if="!analyzing"
        variant="primary"
        size="sm"
        :disabled="!video"
        @click="analyzeCurrent"
      >
        <FluentIcon icon="fluent:play-16-filled" width="14" height="14" />
        {{ t('action.analyze') }}
      </FluentButton>
      <FluentButton v-else size="sm" @click="cancelAnalysis">
        <FluentIcon icon="fluent:stop-16-filled" width="14" height="14" />
        {{ t('action.cancel') }}
      </FluentButton>
      <FluentButton size="sm" :disabled="!library.videos.some((v) => v.status === 'done')" @click="showExport = true">
        <FluentIcon icon="fluent:arrow-export-16-regular" width="14" height="14" />
        {{ t('action.export') }}
      </FluentButton>
    </div>

    <div class="editor-grid">
      <section class="panel media-bin">
        <MediaBinPanel />
      </section>

      <section class="editor-center">
        <div class="panel video-panel">
          <VideoStage />
        </div>
        <div class="panel timeline-wrap">
          <TimelinePanel />
        </div>
      </section>

      <section class="panel inspector">
        <InspectorPanel />
      </section>
    </div>

    <div v-if="dragActive" class="drop-overlay">
      <FluentIcon icon="fluent:arrow-download-24-filled" width="36" height="36" />
      <span>{{ t('empty.title') }}</span>
    </div>

    <ExportDialog v-model="showExport" />
  </div>
</template>

<style scoped>
.analyze-page {
  position: relative;
  padding: 8px 10px 10px;
  gap: 8px;
}
.workspace-head {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 30px;
}
.head-title {
  font-size: 13px;
  font-weight: 600;
  max-width: 40%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.head-status {
  font-size: 11px;
  color: var(--text-muted, #888);
}
.head-status.s-analyzing {
  color: var(--accent, #ea5ec1);
}
.head-status.s-done {
  color: #107c10;
}
.head-status.s-failed {
  color: #c42b1c;
}
.spacer {
  flex: 1;
}
.editor-grid {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: 250px minmax(0, 1fr) 300px;
  gap: 8px;
}
.panel {
  display: flex;
  flex-direction: column;
  min-height: 0;
  border: 1px solid var(--border-subtle, rgba(0, 0, 0, 0.07));
  border-radius: var(--radius-lg, 10px);
  background: var(--bg-card, rgba(255, 255, 255, 0.7));
  padding: 8px;
}
.editor-center {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 0;
  min-height: 0;
}
.video-panel {
  flex: 1 1 auto;
  min-height: 0;
}
.timeline-wrap {
  flex: 0 0 auto;
}
.drop-overlay {
  position: absolute;
  inset: 8px;
  z-index: 50;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  border: 2px dashed var(--accent, #ea5ec1);
  border-radius: 12px;
  background: var(--accent-50, rgba(234, 94, 193, 0.08));
  color: var(--accent, #ea5ec1);
  font-size: 14px;
  font-weight: 600;
  pointer-events: none;
}
</style>
