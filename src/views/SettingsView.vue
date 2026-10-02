<script setup>
import { computed, onMounted, ref } from 'vue'
import { settings, updateAnalyze, updateSettings } from '../stores/settings'
import { api } from '../utils/api'
import { pushToast } from '../components/ToastHost.vue'
import { t } from '../utils/i18n'
import { formatBytes } from '../utils/format'

const cache = ref({ bytes: 0, files: 0 })

const langItems = computed(() => [
  { label: '简体中文', value: 'zh' },
  { label: 'English', value: 'en' }
])
const themeItems = computed(() => [
  { label: t('settings.theme.light'), value: 'light' },
  { label: t('settings.theme.dark'), value: 'dark' }
])
const paletteItems = computed(() => [
  { label: t('settings.palette.peach'), value: 'peach' },
  { label: t('settings.palette.default'), value: 'default' }
])

async function refreshCache () {
  try {
    const stats = await api.cacheStats()
    if (stats) cache.value = stats
  } catch { /* 引擎未就绪 */ }
}

async function clearCache () {
  try {
    await api.clearCache()
    await refreshCache()
    pushToast({ title: t('settings.cache.cleared') })
  } catch (err) {
    pushToast({ title: String(err?.message ?? err) })
  }
}

function resetAnalysis () {
  updateAnalyze({
    sensitivity: 50,
    minDuration: 0.4,
    mergeGap: 0.3,
    compensateCamera: true,
    markSceneCuts: true,
    speed: 'standard',
    fps: 8,
    preset: 'custom'
  })
}

onMounted(refreshCache)
</script>

<template>
  <div class="page">
    <h2 class="page-title">{{ t('settings.title') }}</h2>
    <div class="page-scroll settings-scroll">
      <FluentSettingsCard :title="t('settings.appearance')" icon="fluent:color-24-regular">
        <div class="form-row">
          <span>{{ t('settings.lang') }}</span>
          <FluentSegmented
            :items="langItems"
            :model-value="settings.lang"
            @update:model-value="(v) => updateSettings({ lang: v })"
          />
        </div>
        <div class="form-row">
          <span>{{ t('settings.theme') }}</span>
          <FluentSegmented
            :items="themeItems"
            :model-value="settings.theme"
            @update:model-value="(v) => updateSettings({ theme: v })"
          />
        </div>
        <div class="form-row">
          <span>{{ t('settings.palette') }}</span>
          <FluentSegmented
            :items="paletteItems"
            :model-value="settings.palette"
            @update:model-value="(v) => updateSettings({ palette: v })"
          />
        </div>
      </FluentSettingsCard>

      <FluentSettingsCard :title="t('settings.analysis')" icon="fluent:options-24-regular">
        <div class="form-row">
          <span>{{ t('params.sensitivity') }}</span>
          <div class="slider-wrap">
            <FluentSlider
              :model-value="settings.analyze.sensitivity"
              :min="0"
              :max="100"
              :step="1"
              @update:model-value="(v) => updateAnalyze({ sensitivity: v })"
            />
          </div>
        </div>
        <div class="form-row">
          <span>{{ t('params.minDuration') }}</span>
          <FluentNumberBox
            class="num-box"
            :model-value="settings.analyze.minDuration"
            :min="0.1"
            :max="10"
            :step="0.1"
            @update:model-value="(v) => updateAnalyze({ minDuration: v })"
          />
        </div>
        <div class="form-row">
          <span>{{ t('params.mergeGap') }}</span>
          <FluentNumberBox
            class="num-box"
            :model-value="settings.analyze.mergeGap"
            :min="0"
            :max="5"
            :step="0.1"
            @update:model-value="(v) => updateAnalyze({ mergeGap: v })"
          />
        </div>
        <div class="form-row">
          <span>{{ t('params.compensate') }}</span>
          <FluentToggleSwitch
            :model-value="settings.analyze.compensateCamera"
            @update:model-value="(v) => updateAnalyze({ compensateCamera: v })"
          />
        </div>
        <div class="form-row">
          <span>{{ t('params.markCuts') }}</span>
          <FluentToggleSwitch
            :model-value="settings.analyze.markSceneCuts"
            @update:model-value="(v) => updateAnalyze({ markSceneCuts: v })"
          />
        </div>
        <div class="form-row">
          <span />
          <FluentButton size="sm" @click="resetAnalysis">{{ t('action.reset') }}</FluentButton>
        </div>
      </FluentSettingsCard>

      <FluentSettingsCard :title="t('settings.storage')" icon="fluent:hard-drive-24-regular">
        <div class="form-row">
          <span>{{ t('settings.cache') }}</span>
          <span class="mono cache-text">
            {{ t('settings.cache.stats', { size: formatBytes(cache.bytes), n: cache.files }) }}
          </span>
        </div>
        <div class="form-row">
          <span />
          <div class="cache-actions">
            <FluentButton size="sm" @click="refreshCache">↻</FluentButton>
            <FluentButton size="sm" @click="clearCache">{{ t('settings.cache.clear') }}</FluentButton>
          </div>
        </div>
      </FluentSettingsCard>
    </div>
  </div>
</template>

<style scoped>
.settings-scroll {
  display: flex;
  flex-direction: column;
  gap: 12px;
  max-width: 720px;
}
.slider-wrap {
  width: 220px;
}
.num-box {
  width: 120px;
}
.cache-text {
  color: var(--text-secondary, #555);
  font-size: 12px;
}
.cache-actions {
  display: flex;
  gap: 6px;
}
</style>
