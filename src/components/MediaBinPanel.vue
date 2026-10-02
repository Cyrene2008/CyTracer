<script setup>
import { computed } from 'vue'
import {
  analyzeAll,
  clearVideos,
  importViaDialog,
  library,
  removeVideo,
  selectVideo
} from '../stores/library'
import { t } from '../utils/i18n'
import { formatDuration } from '../utils/format'

const count = computed(() => library.videos.length)

const statusLabel = (video) => {
  switch (video.status) {
    case 'probing': return t('status.probing')
    case 'analyzing': return t('status.analyzing')
    case 'done': return t('status.done')
    case 'failed': return t('status.failed')
    case 'canceled': return t('status.canceled')
    default: return t('status.pending')
  }
}

const metaLine = (video) => {
  if (!video.info) return ''
  const parts = []
  if (video.info.duration) parts.push(formatDuration(video.info.duration))
  if (video.info.width) parts.push(`${video.info.width}x${video.info.height}`)
  if (video.info.videoCodec) parts.push(video.info.videoCodec.toUpperCase())
  return parts.join(' · ')
}

const pendingCount = computed(() =>
  library.videos.filter((v) => v.status !== 'done' && v.status !== 'analyzing').length
)
</script>

<template>
  <div class="bin">
    <div class="bin-head">
      <span class="section-title">{{ t('library.title') }}</span>
      <span class="count">{{ t('library.count', { n: count }) }}</span>
    </div>

    <div class="bin-list">
      <div v-if="!count" class="empty grow-area">
        <FluentIcon icon="fluent:video-clip-multiple-24-regular" width="28" height="28" />
        <span>{{ t('library.empty') }}</span>
      </div>
      <div
        v-for="video in library.videos"
        :key="video.id"
        class="bin-item"
        :class="{ active: video.id === library.currentId }"
        @click="selectVideo(video.id)"
      >
        <div class="bin-item-main">
          <div class="bin-name" :title="video.path">{{ video.name }}</div>
          <div class="bin-meta">
            <span>{{ metaLine(video) }}</span>
          </div>
          <div v-if="video.status === 'analyzing'" class="bin-progress">
            <FluentProgressBar :value="Math.round((video.progress || 0) * 100)" :max="100" />
          </div>
          <div v-if="video.error" class="bin-error" :title="video.error">{{ video.error }}</div>
        </div>
        <div class="bin-side">
          <span class="status-chip" :class="`s-${video.status}`">{{ statusLabel(video) }}</span>
          <button class="icon-btn" :title="t('action.remove')" @click.stop="removeVideo(video.id)">
            <FluentIcon icon="fluent:dismiss-16-regular" width="12" height="12" />
          </button>
        </div>
      </div>
    </div>

    <div class="bin-actions">
      <FluentButton variant="primary" size="sm" @click="importViaDialog">
        <FluentIcon icon="fluent:add-16-filled" width="14" height="14" />
        {{ t('action.import') }}
      </FluentButton>
      <FluentButton
        size="sm"
        :disabled="!pendingCount || library.queueRunning"
        @click="analyzeAll"
      >
        <FluentIcon icon="fluent:play-16-regular" width="14" height="14" />
        {{ t('action.analyze') }}
      </FluentButton>
      <FluentButton size="sm" variant="subtle" :disabled="!count" @click="clearVideos">
        {{ t('action.clear') }}
      </FluentButton>
    </div>
  </div>
</template>

<style scoped>
.bin {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
}
.bin-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
}
.count {
  font-size: 11px;
  color: var(--text-muted, #888);
}
.bin-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 2px 0;
}
.bin-item {
  display: flex;
  gap: 6px;
  padding: 7px 8px;
  border-radius: 8px;
  border: 1px solid transparent;
  cursor: pointer;
  transition: background var(--duration-fast, 0.15s);
}
.bin-item:hover {
  background: var(--bg-hover, rgba(0, 0, 0, 0.04));
}
.bin-item.active {
  background: var(--accent-50, rgba(234, 94, 193, 0.09));
  border-color: var(--accent-200, rgba(234, 94, 193, 0.3));
}
.bin-item-main {
  flex: 1;
  min-width: 0;
}
.bin-name {
  font-size: 12px;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.bin-meta {
  font-size: 11px;
  color: var(--text-muted, #888);
  margin-top: 2px;
}
.bin-progress {
  margin-top: 5px;
}
.bin-error {
  margin-top: 4px;
  font-size: 11px;
  color: var(--text-secondary, #777);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.bin-side {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 4px;
}
.status-chip {
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 4px;
  background: var(--bg-hover, rgba(0, 0, 0, 0.06));
  color: var(--text-secondary, #666);
  white-space: nowrap;
}
.status-chip.s-analyzing {
  background: var(--accent-50, rgba(234, 94, 193, 0.12));
  color: var(--accent, #ea5ec1);
}
.status-chip.s-done {
  background: rgba(16, 124, 16, 0.12);
  color: #107c10;
}
.status-chip.s-failed {
  background: rgba(196, 43, 28, 0.12);
  color: #c42b1c;
}
.icon-btn {
  border: none;
  background: transparent;
  color: var(--text-muted, #888);
  cursor: pointer;
  padding: 2px;
  border-radius: 4px;
  line-height: 1;
}
.icon-btn:hover {
  background: var(--bg-hover, rgba(0, 0, 0, 0.08));
  color: var(--text-primary, #222);
}
.bin-actions {
  display: flex;
  gap: 6px;
  padding-top: 8px;
  border-top: 1px solid var(--border-subtle, rgba(0, 0, 0, 0.06));
}
</style>
