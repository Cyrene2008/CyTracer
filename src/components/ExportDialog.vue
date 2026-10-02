<script setup>
import { computed, ref } from 'vue'
import { analyzedVideos, currentVideo, exportClips, exportContactSheet, exportMarkers } from '../stores/library'
import { settings, updateSettings } from '../stores/settings'
import { t } from '../utils/i18n'

const props = defineProps({
  modelValue: { type: Boolean, default: false }
})
const emit = defineEmits(['update:modelValue'])

const format = ref('json')
const scope = ref('current')
const includeCuts = ref(true)
const exportMode = ref('reel')
const busy = ref(false)

const modeItems = computed(() => [
  { label: t('export.mode.reel'), value: 'reel' },
  { label: t('export.mode.both'), value: 'both' },
  { label: t('export.mode.clips'), value: 'clips' }
])

const formatItems = computed(() => [
  { label: 'JSON', value: 'json' },
  { label: 'CSV', value: 'csv' },
  { label: 'SRT', value: 'srt' },
  { label: t('export.sheet'), value: 'sheet' },
  { label: t('export.clips'), value: 'clips' }
])
const scopeItems = computed(() => [
  { label: t('export.scope.current'), value: 'current' },
  { label: t('export.scope.all'), value: 'all' }
])
const available = computed(() => scope.value === 'all'
  ? analyzedVideos.value.length
  : (currentVideo.value?.status === 'done' ? 1 : 0))

function close (value = false) {
  emit('update:modelValue', value)
}

async function confirm () {
  busy.value = true
  try {
    if (format.value === 'sheet') {
      await exportContactSheet({
        scope: scope.value,
        includeCuts: includeCuts.value
      })
    } else if (format.value === 'clips') {
      await exportClips({
        scope: scope.value,
        includeCuts: includeCuts.value,
        mode: exportMode.value
      })
    } else {
      await exportMarkers({
        format: format.value,
        scope: scope.value,
        includeCuts: includeCuts.value
      })
    }
    close(false)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <FluentContentDialog
    :model-value="props.modelValue"
    :title="t('export.title')"
    :primary-button-text="t('action.export')"
    :secondary-button-text="t('action.close')"
    :primary-button-disabled="busy || !available"
    @update:model-value="close"
    @primary-click="confirm"
    @secondary-click="close(false)"
  >
    <div class="export-body">
      <div class="field">
        <span class="field-label">{{ t('export.format') }}</span>
        <FluentSegmented v-model="format" class="field-control" :items="formatItems" />
        <span v-if="format === 'sheet'" class="field-hint">{{ t('export.sheet.hint') }}</span>
        <span v-else-if="format === 'clips'" class="field-hint">{{ t('export.clips.hint') }}</span>
      </div>
      <div v-if="format === 'clips'" class="field">
        <span class="field-label">{{ t('export.mode') }}</span>
        <FluentSegmented v-model="exportMode" class="field-control" :items="modeItems" />
      </div>
      <div v-if="format === 'clips'" class="field inline">
        <span class="field-label">{{ t('export.pad') }}</span>
        <FluentNumberBox
          class="pad-box"
          :model-value="settings.exportPad ?? 0.3"
          :min="0"
          :max="10"
          :step="0.1"
          @update:model-value="(v) => updateSettings({ exportPad: v })"
        />
      </div>
      <div class="field">
        <span class="field-label">{{ t('export.scope') }}</span>
        <FluentSegmented v-model="scope" class="field-control" :items="scopeItems" />
      </div>
      <div class="field inline">
        <span class="field-label">{{ t('export.includeCuts') }}</span>
        <FluentToggleSwitch v-model="includeCuts" />
      </div>
    </div>
  </FluentContentDialog>
</template>

<style scoped>
.export-body {
  display: flex;
  flex-direction: column;
  gap: 14px;
  min-width: 400px;
  max-width: 100%;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
}
.field.inline {
  flex-direction: row;
  align-items: center;
  justify-content: space-between;
}
.field-label {
  font-size: 12px;
  color: var(--text-secondary, #666);
}
.field-hint {
  font-size: 11px;
  color: var(--text-muted, #888);
}
.field-control {
  width: 100%;
  min-width: 0;
}
.field-control :deep(.segmented-items) {
  width: 100%;
}
.field-control :deep(.segmented-item) {
  flex: 1 1 0;
  min-width: 0;
  padding: 6px 8px;
}
.field-control :deep(.segmented-item-label) {
  display: block;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.pad-box {
  width: 110px;
}
</style>
