<script setup>
import { computed, ref } from 'vue'
import TimelineCanvas from './TimelineCanvas.vue'
import { addMarkerAt, applyParams, currentVideo, deleteEvent, library, mergeWithNext, selectedEvent } from '../../stores/library'
import { player, requestSeek } from '../../stores/player'
import { settings, updateAnalyze } from '../../stores/settings'
import { t } from '../../utils/i18n'

const canvasRef = ref(null)

const eventCount = computed(() => currentVideo.value?.events.length || 0)
const video = computed(() => currentVideo.value)

const speedItems = computed(() => [
  { label: t('params.speed.fast'), value: 'fast' },
  { label: t('params.speed.standard'), value: 'standard' },
  { label: t('params.speed.fine'), value: 'fine' }
])

const fpsMap = { fast: 4, standard: 8, fine: 12 }

function onSensitivityChange (value) {
  updateAnalyze({ sensitivity: value, preset: 'custom' })
  applyParams()
}

function onSpeedChange (value) {
  updateAnalyze({ speed: value, fps: fpsMap[value] || 8, preset: 'custom' })
  applyParams()
}

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
      <span class="inline-field">
        <span class="label">{{ t('params.sensitivity') }}</span>
        <FluentSlider
          class="sensitivity"
          :model-value="settings.analyze.sensitivity"
          :min="0"
          :max="100"
          :step="1"
          :show-value="false"
          @update:model-value="(v) => updateAnalyze({ sensitivity: v })"
          @change="onSensitivityChange"
        />
      </span>
      <FluentSegmented
        :items="speedItems"
        :model-value="settings.analyze.speed"
        @update:model-value="onSpeedChange"
      />
      <span class="stats mono">
        {{ t('stats.events') }} {{ eventCount }}
        <template v-if="video?.frameCount"> · {{ video.frameCount }}f</template>
        <template v-if="video?.noiseFloor"> · floor {{ video.noiseFloor.toFixed(3) }}</template>
      </span>
    </div>
    <TimelineCanvas ref="canvasRef" />
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
.inline-field {
  display: flex;
  align-items: center;
  gap: 6px;
}
.sensitivity {
  width: 130px;
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
</style>
