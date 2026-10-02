<script setup>
import { computed, ref } from 'vue'
import TimelineCanvas from './TimelineCanvas.vue'
import { addMarkerAt, currentVideo, deleteEvent, library, mergeWithNext, selectedEvent } from '../../stores/library'
import { player, requestSeek } from '../../stores/player'
import { t } from '../../utils/i18n'

const canvasRef = ref(null)

const eventCount = computed(() => currentVideo.value?.events.length || 0)
const video = computed(() => currentVideo.value)

function addAtPlayhead () {
  addMarkerAt(player.currentTime, Math.min(1, (video.value?.duration || 4) / 10))
  canvasRef.value?.requestDraw()
}

function removeSelected () {
  if (library.selectedEventId) {
    deleteEvent(library.selectedEventId)
    canvasRef.value?.requestDraw()
  }
}

function mergeSelected () {
  if (library.selectedEventId) {
    mergeWithNext(library.selectedEventId)
    canvasRef.value?.requestDraw()
  }
}

function gotoSelected () {
  if (selectedEvent.value) requestSeek(selectedEvent.value.peak || selectedEvent.value.start)
}
</script>

<template>
  <div class="timeline-panel">
    <div class="tl-toolbar">
      <FluentButton size="sm" variant="subtle" icon-only :title="t('timeline.zoomOut')" @click="canvasRef?.zoom(1.4)">
        <FluentIcon icon="fluent:zoom-out-24-regular" width="15" height="15" />
      </FluentButton>
      <FluentButton size="sm" variant="subtle" icon-only :title="t('timeline.zoomIn')" @click="canvasRef?.zoom(0.7)">
        <FluentIcon icon="fluent:zoom-in-24-regular" width="15" height="15" />
      </FluentButton>
      <FluentButton size="sm" variant="subtle" icon-only :title="t('timeline.fit')" @click="canvasRef?.fit()">
        <FluentIcon icon="fluent:arrow-fit-24-regular" width="15" height="15" />
      </FluentButton>
      <span class="divider" />
      <FluentButton size="sm" variant="subtle" icon-only :title="t('action.addMarker')" :disabled="!video" @click="addAtPlayhead">
        <FluentIcon icon="fluent:add-16-regular" width="15" height="15" />
      </FluentButton>
      <FluentButton size="sm" variant="subtle" icon-only :title="t('marker.goto')" :disabled="!selectedEvent" @click="gotoSelected">
        <FluentIcon icon="fluent:arrow-next-24-regular" width="15" height="15" />
      </FluentButton>
      <FluentButton size="sm" variant="subtle" icon-only :title="t('action.merge')" :disabled="!selectedEvent" @click="mergeSelected">
        <FluentIcon icon="fluent:merge-24-regular" width="15" height="15" />
      </FluentButton>
      <FluentButton size="sm" variant="subtle" icon-only :title="t('action.delete')" :disabled="!selectedEvent" @click="removeSelected">
        <FluentIcon icon="fluent:delete-24-regular" width="15" height="15" />
      </FluentButton>
      <span class="spacer" />
      <span v-if="video" class="stats mono">
        {{ t('stats.events') }} {{ eventCount }}
        <template v-if="video.frameCount"> · {{ video.frameCount }}f</template>
        <template v-if="video.noiseFloor"> · floor {{ video.noiseFloor.toFixed(3) }}</template>
      </span>
      <span v-if="video" class="zoom-label mono" title="可见时间窗口">{{ (canvasRef?.viewSpan ?? 0).toFixed(1) }}s</span>
    </div>
    <div class="timeline-body">
      <TimelineCanvas ref="canvasRef" />
      <div v-if="!video" class="tl-empty">
        <FluentIcon icon="fluent:timeline-24-regular" width="22" height="22" />
        <span>{{ t('timeline.empty') }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.timeline-panel {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
}
.tl-toolbar {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-wrap: wrap;
  padding-bottom: 6px;
}
.divider {
  width: 1px;
  height: 18px;
  background: var(--border-subtle, rgba(0, 0, 0, 0.1));
  margin: 0 4px;
}
.spacer {
  flex: 1;
}
.timeline-body {
  position: relative;
  flex: 1;
  min-height: 0;
}
.tl-empty {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  color: var(--text-muted, #888);
  font-size: 12px;
  pointer-events: none;
}
.label {
  font-size: 11px;
  color: var(--text-secondary, #666);
}
.stats {
  font-size: 11px;
  color: var(--text-muted, #888);
  white-space: nowrap;
}
.zoom-label {
  font-size: 11px;
  color: var(--accent, #ea5ec1);
  background: var(--accent-50, rgba(234, 94, 193, 0.1));
  border-radius: 4px;
  padding: 0 5px;
  white-space: nowrap;
}
</style>
