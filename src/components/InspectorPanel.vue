<script setup>
import { computed } from 'vue'
import {
  applyParams,
  currentVideo,
  deleteEvent,
  library,
  mergeWithNext,
  selectedEvent,
  updateEvent
} from '../stores/library'
import { requestSeek } from '../stores/player'
import { settings, updateAnalyze } from '../stores/settings'
import { t } from '../utils/i18n'
import { formatTime, parseTime } from '../utils/format'

const video = computed(() => currentVideo.value)
const selected = computed(() => selectedEvent.value)

const speedItems = computed(() => [
  { label: t('params.speed.fast'), value: 'fast' },
  { label: t('params.speed.standard'), value: 'standard' },
  { label: t('params.speed.fine'), value: 'fine' }
])

const fpsMap = { fast: 4, standard: 8, fine: 12 }

function onSpeedChange (value) {
  updateAnalyze({ speed: value, fps: fpsMap[value] || 8, preset: 'custom' })
  applyParams()
}

const levelItems = computed(() => [
  { label: t('marker.level.low'), value: 0 },
  { label: t('marker.level.mid'), value: 1 },
  { label: t('marker.level.high'), value: 2 }
])

const kindLabel = computed(() => {
  if (!selected.value) return ''
  if (selected.value.kind === 'cut') return t('marker.type.cut')
  if (selected.value.kind === 'manual') return t('marker.type.manual')
  return t('marker.type.motion')
})

function patch (key, value) {
  if (!selected.value) return
  updateEvent(selected.value.id, { [key]: value })
}

function onTimeCommit (key, text) {
  const value = parseTime(text)
  if (Number.isFinite(value)) patch(key, Math.max(0, value))
}

function onParamChange () {
  applyParams()
}

function removeSelected () {
  if (selected.value) deleteEvent(selected.value.id)
}

function mergeSelected () {
  if (selected.value) mergeWithNext(selected.value.id)
}

function goto () {
  if (selected.value) requestSeek(selected.value.start)
}
</script>

<template>
  <div class="inspector">
    <div class="section">
      <h3 class="section-title">{{ t('params.title') }}</h3>
      <div class="form-row">
        <span>{{ t('params.speed') }}</span>
        <FluentSegmented
          :items="speedItems"
          :model-value="settings.analyze.speed"
          @update:model-value="onSpeedChange"
        />
      </div>
      <div class="form-row">
        <span>{{ t('params.sensitivity') }}</span>
        <div class="row-value">
          <FluentSlider
            class="param-slider"
            :model-value="settings.analyze.sensitivity"
            :min="0"
            :max="100"
            :step="1"
            :show-value="true"
            @update:model-value="(v) => updateAnalyze({ sensitivity: v })"
            @change="onParamChange"
          />
        </div>
      </div>
      <div class="form-row">
        <span>{{ t('params.minDuration') }}</span>
        <FluentNumberBox
          class="param-num"
          :model-value="settings.analyze.minDuration"
          :min="0.1"
          :max="10"
          :step="0.1"
          @update:model-value="(v) => updateAnalyze({ minDuration: v })"
          @change="onParamChange"
        />
      </div>
      <div class="form-row">
        <span>{{ t('params.mergeGap') }}</span>
        <FluentNumberBox
          class="param-num"
          :model-value="settings.analyze.mergeGap"
          :min="0"
          :max="5"
          :step="0.1"
          @update:model-value="(v) => updateAnalyze({ mergeGap: v })"
          @change="onParamChange"
        />
      </div>
      <div class="form-row">
        <span :title="t('params.compensate.desc')">{{ t('params.compensate') }}</span>
        <FluentToggleSwitch
          :model-value="settings.analyze.compensateCamera"
          @update:model-value="(v) => { updateAnalyze({ compensateCamera: v }); onParamChange() }"
        />
      </div>
      <div class="form-row">
        <span>{{ t('params.markCuts') }}</span>
        <FluentToggleSwitch
          :model-value="settings.analyze.markSceneCuts"
          @update:model-value="(v) => { updateAnalyze({ markSceneCuts: v }); onParamChange() }"
        />
      </div>
      <p v-if="video && video.status !== 'done'" class="hint">{{ t('status.pending') }} — {{ t('action.analyze') }}</p>
    </div>

    <div class="section grow">
      <h3 class="section-title">{{ t('marker.title') }}</h3>
      <template v-if="selected">
        <div class="form-row">
          <span>{{ t('marker.type') }}</span>
          <span class="value">
            {{ kindLabel }}
            <span v-if="selected.auto && !selected.edited" class="tag">{{ t('marker.auto') }}</span>
            <span v-else-if="selected.edited" class="tag edited">{{ t('marker.edited') }}</span>
          </span>
        </div>
        <div class="form-row">
          <span>{{ t('marker.start') }}</span>
          <FluentTextBox
            class="time-box"
            :model-value="formatTime(selected.start)"
            @change="(v) => onTimeCommit('start', v)"
          />
        </div>
        <div class="form-row">
          <span>{{ t('marker.end') }}</span>
          <FluentTextBox
            class="time-box"
            :model-value="formatTime(selected.end)"
            @change="(v) => onTimeCommit('end', v)"
          />
        </div>
        <div class="form-row">
          <span>{{ t('marker.peak') }}</span>
          <FluentTextBox
            class="time-box"
            :model-value="formatTime(selected.peak)"
            @change="(v) => onTimeCommit('peak', v)"
          />
        </div>
        <div v-if="selected.kind !== 'cut'" class="form-row">
          <span>{{ t('marker.level') }}</span>
          <FluentSegmented
            :items="levelItems"
            :model-value="selected.level"
            @update:model-value="(v) => patch('level', v)"
          />
        </div>
        <div class="form-row">
          <span>{{ t('marker.label') }}</span>
          <FluentTextBox
            class="grow-box"
            :model-value="selected.label"
            :placeholder="kindLabel"
            @update:model-value="(v) => patch('label', v)"
          />
        </div>
        <div class="form-row">
          <span>{{ t('marker.note') }}</span>
          <FluentTextBox
            class="grow-box"
            :model-value="selected.note"
            @update:model-value="(v) => patch('note', v)"
          />
        </div>
        <div class="marker-actions">
          <FluentButton size="sm" @click="goto">{{ t('marker.goto') }}</FluentButton>
          <FluentButton size="sm" @click="mergeSelected">{{ t('action.merge') }}</FluentButton>
          <FluentButton size="sm" variant="danger" @click="removeSelected">{{ t('action.delete') }}</FluentButton>
        </div>
      </template>
      <div v-else class="empty grow">
        <FluentIcon icon="fluent:tag-24-regular" width="26" height="26" />
        <span>{{ t('marker.none') }}</span>
      </div>
    </div>

    <div v-if="video?.info" class="section stats-section">
      <div class="form-row">
        <span>{{ t('stats.duration') }}</span>
        <span class="mono">{{ formatTime(video.info.duration) }}</span>
      </div>
      <div class="form-row">
        <span>{{ t('stats.resolution') }}</span>
        <span class="mono">{{ video.info.width }}x{{ video.info.height }}</span>
      </div>
      <div class="form-row">
        <span>{{ t('stats.codec') }}</span>
        <span class="mono">{{ (video.info.videoCodec || '').toUpperCase() }}</span>
      </div>
      <div class="form-row">
        <span>{{ t('stats.fps') }}</span>
        <span class="mono">{{ (video.info.fps || 0).toFixed(2) }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.inspector {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-height: 0;
  height: 100%;
  overflow-y: auto;
}
.section {
  border-bottom: 1px solid var(--border-subtle, rgba(0, 0, 0, 0.06));
  padding-bottom: 8px;
}
.section:last-child {
  border-bottom: none;
}
.grow {
  flex: 1;
  min-height: 0;
}
.form-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 3px 0;
  font-size: 12px;
}
.row-value {
  flex: 1;
  max-width: 150px;
}
.param-slider {
  width: 100%;
}
.row-value :deep(.fluent-slider) {
  min-width: 0;
}
.param-num {
  width: 92px;
}
.time-box {
  width: 116px;
}
.grow-box {
  flex: 1;
  min-width: 0;
}
.value {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: var(--text-secondary, #555);
}
.tag {
  font-size: 10px;
  padding: 0 4px;
  border-radius: 3px;
  background: var(--bg-hover, rgba(0, 0, 0, 0.06));
  color: var(--text-muted, #888);
}
.tag.edited {
  background: rgba(16, 124, 16, 0.12);
  color: #107c10;
}
.marker-actions {
  display: flex;
  gap: 6px;
  margin-top: 8px;
}
.hint {
  margin: 6px 0 0;
  font-size: 11px;
  color: var(--text-muted, #888);
}
.stats-section {
  font-size: 11px;
}
</style>
