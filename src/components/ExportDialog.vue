<script setup>
import { computed, ref } from 'vue'
import { analyzedVideos, currentVideo, exportMarkers } from '../stores/library'
import { t } from '../utils/i18n'

const props = defineProps({
  modelValue: { type: Boolean, default: false }
})
const emit = defineEmits(['update:modelValue'])

const format = ref('json')
const scope = ref('current')
const includeCuts = ref(true)
const busy = ref(false)

const formatItems = computed(() => [
  { label: 'JSON', value: 'json' },
  { label: 'CSV', value: 'csv' },
  { label: 'SRT', value: 'srt' }
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
    await exportMarkers({
      format: format.value,
      scope: scope.value,
      includeCuts: includeCuts.value
    })
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
      <div class="form-row">
        <span>{{ t('export.format') }}</span>
        <FluentSegmented v-model="format" :items="formatItems" />
      </div>
      <div class="form-row">
        <span>{{ t('export.scope') }}</span>
        <FluentSegmented v-model="scope" :items="scopeItems" />
      </div>
      <div class="form-row">
        <span>{{ t('export.includeCuts') }}</span>
        <FluentToggleSwitch v-model="includeCuts" />
      </div>
    </div>
  </FluentContentDialog>
</template>

<style scoped>
.export-body {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-width: 340px;
}
</style>
