<script setup>
import { computed } from 'vue'
import { t } from '../utils/i18n'
import { backendState } from '../stores/backend'
import logo from '../assets/logo.png'
import avatar from '../assets/cyrene2008.png'

const commit = typeof __BUILD_COMMIT__ !== 'undefined' ? __BUILD_COMMIT__ : 'unknown'
const version = computed(() => backendState.version || '26.0.1')

const REPO_URL = 'https://github.com/Cyrene2008/CyTracer'
const AUTHOR_URL = 'https://github.com/Cyrene2008'
const FLUENT_URL = 'https://fluent.cyrene.hk'

async function open (url) {
  try {
    const { openUrl } = await import('@tauri-apps/plugin-opener')
    await openUrl(url)
  } catch {
    window.open(url, '_blank')
  }
}
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
          <div class="form-row">
            <span>{{ t('about.repository') }}</span>
            <button class="link" @click="open(REPO_URL)">
              <FluentIcon icon="fluent:folder-link-24-regular" width="14" height="14" />
              github.com/Cyrene2008/CyTracer
            </button>
          </div>
        </div>
      </FluentCard>

      <FluentCard class="about-card author-card" material="solid">
        <div class="author-head">
          <img class="author-avatar" :src="avatar" alt="Cyrene2008" draggable="false" />
          <div class="author-info">
            <div class="author-name">
              Cyrene2008
              <span class="author-role">{{ t('about.author') }}</span>
            </div>
            <button class="link" @click="open(AUTHOR_URL)">
              <FluentIcon icon="fluent:code-24-regular" width="14" height="14" />
              GitHub · Cyrene2008
            </button>
          </div>
        </div>
      </FluentCard>

      <FluentCard class="about-card powered-card" material="solid">
        <div class="powered-title">Powered by VueFluentWidgets</div>
        <div class="powered-sub">Fluent Design System for Vue · MIT License</div>
        <div class="powered-sub">Copyright © 2025-2026 Cyrene2008</div>
        <button class="link" @click="open(FLUENT_URL)">
          <FluentIcon icon="fluent:globe-24-regular" width="14" height="14" />
          {{ FLUENT_URL.replace('https://', '') }}
        </button>
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
.link {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  border: none;
  background: transparent;
  color: var(--accent, #ea5ec1);
  font-size: 12px;
  font-family: var(--font-ui);
  padding: 2px 4px;
  border-radius: 4px;
  cursor: pointer;
}
.link:hover {
  background: var(--accent-50, rgba(234, 94, 193, 0.1));
  text-decoration: underline;
}
.author-head {
  display: flex;
  align-items: center;
  gap: 14px;
}
.author-avatar {
  width: 56px;
  height: 56px;
  border-radius: 50%;
  object-fit: cover;
  border: 2px solid var(--accent-200, rgba(234, 94, 193, 0.3));
  -webkit-user-drag: none;
}
.author-name {
  font-size: 16px;
  font-weight: 600;
  display: flex;
  align-items: baseline;
  gap: 8px;
}
.author-role {
  font-size: 11px;
  font-weight: 400;
  color: var(--text-muted, #888);
  border: 1px solid var(--border-strong, rgba(0, 0, 0, 0.15));
  border-radius: 4px;
  padding: 0 4px;
}
.powered-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  text-align: center;
}
.powered-title {
  font-size: 15px;
  font-weight: 600;
}
.powered-sub {
  font-size: 12px;
  color: var(--text-secondary, #666);
}
</style>
