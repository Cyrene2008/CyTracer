<script setup>
import { computed, ref } from 'vue'
import { t } from '../utils/i18n'
import { backendState } from '../stores/backend'

const commit = typeof __BUILD_COMMIT__ !== 'undefined' ? __BUILD_COMMIT__ : 'unknown'
const version = computed(() => backendState.version || '26.0.0')
const updateState = ref('')
const latestVersion = ref('')

function compareVersions (a, b) {
  const pa = String(a).split('.').map(Number)
  const pb = String(b).split('.').map(Number)
  for (let i = 0; i < Math.max(pa.length, pb.length); i += 1) {
    const diff = (pa[i] || 0) - (pb[i] || 0)
    if (diff !== 0) return diff
  }
  return 0
}

async function checkUpdate () {
  updateState.value = 'checking'
  try {
    const resp = await fetch('https://api.github.com/repos/Cyrene2008/CyTracer/releases/latest', {
      headers: { Accept: 'application/vnd.github+json' }
    })
    if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
    const data = await resp.json()
    const latest = String(data.tag_name || '').replace(/^v/, '')
    if (latest && compareVersions(latest, version.value) > 0) {
      latestVersion.value = latest
      updateState.value = 'new'
    } else {
      updateState.value = 'latest'
    }
  } catch {
    updateState.value = 'error'
  }
}

function openReleases () {
  window.open('https://github.com/Cyrene2008/CyTracer/releases/latest', '_blank')
}
</script>

<template>
  <div class="page">
    <h2 class="page-title">{{ t('about.title') }}</h2>
    <div class="page-scroll about-scroll">
      <FluentCard class="about-card">
        <div class="about-head">
          <div class="about-logo">Cy</div>
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
            <span class="mono">FFmpeg · VueFluentWidgets</span>
          </div>
        </div>
        <div class="about-actions">
          <FluentButton variant="primary" size="sm" :disabled="updateState === 'checking'" @click="checkUpdate">
            {{ t('about.checkUpdate') }}
          </FluentButton>
          <template v-if="updateState === 'latest'">
            <span class="update-hint">{{ t('about.upToDate') }}</span>
          </template>
          <template v-else-if="updateState === 'new'">
            <span class="update-hint">{{ t('about.newVersion', { version: latestVersion }) }}</span>
            <FluentButton size="sm" @click="openReleases">{{ t('about.openRelease') }}</FluentButton>
          </template>
          <template v-else-if="updateState === 'error'">
            <FluentButton size="sm" @click="openReleases">GitHub Releases</FluentButton>
          </template>
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
  display: grid;
  place-items: center;
  background: var(--accent, #ea5ec1);
  color: #fff;
  font-size: 22px;
  font-weight: 700;
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
.about-actions {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-top: 8px;
}
.update-hint {
  color: var(--text-muted, #888);
  font-size: 12px;
}
</style>
