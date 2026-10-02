<script setup>
import { computed } from 'vue'
import { t } from '../utils/i18n'
import { backendState } from '../stores/backend'
import logo from '../assets/logo.png'

const commit = typeof __BUILD_COMMIT__ !== 'undefined' ? __BUILD_COMMIT__ : 'unknown'
const version = computed(() => backendState.version || '26.0.0')
</script>

<template>
  <div class="page">
    <h2 class="page-title">{{ t('about.title') }}</h2>
    <div class="page-scroll about-scroll">
      <FluentCard class="about-card" material="solid">
        <div class="about-head">
          <img class="about-logo" :src="logo" alt="CyTracer" draggable="false" />
          <div>
            <div class="about-name">CyTracer</div>
            <div class="about-sub">Cyreneの视频分析器</div>
          </div>
        </div>
        <p class="about-desc">{{ t('about.desc') }}</p>
        <div class="about-meta">
          <div class="form-row">
            <span>{{ t('about.version') }}</span>
            <span class="mono">v{{ version }}</span>
          </div>
          <div class="form-row">
            <span>{{ t('about.commit') }}</span>
            <span class="mono">{{ commit }}</span>
          </div>
          <div class="form-row">
            <span>{{ t('about.license') }}</span>
            <span class="mono">GPL-3.0-or-later</span>
          </div>
          <div class="form-row">
            <span>{{ t('about.thirdParty') }}</span>
            <span class="mono">FFmpeg · VueFluentWidgets · MiSans</span>
          </div>
        </div>
      </FluentCard>
    </div>
  </div>
</template>

<style scoped>
.about-scroll {
  display: flex;
  flex-direction: column;
  gap: 12px;
  max-width: 640px;
}
.about-card {
  padding: 20px;
}
.about-head {
  display: flex;
  align-items: center;
  gap: 14px;
}
.about-logo {
  width: 52px;
  height: 52px;
  border-radius: 14px;
  -webkit-user-drag: none;
}
.about-name {
  font-size: 20px;
  font-weight: 600;
}
.about-sub {
  color: var(--text-secondary, #555);
}
.about-desc {
  color: var(--text-secondary, #555);
  line-height: 1.7;
  margin: 14px 0;
}
.about-meta {
  border-top: 1px solid var(--border-subtle, rgba(0, 0, 0, 0.07));
  padding-top: 8px;
}
</style>
