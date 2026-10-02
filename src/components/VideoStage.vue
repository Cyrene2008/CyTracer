<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { currentVideo } from '../stores/library'
import { player, requestTogglePlay, requestStep } from '../stores/player'
import { api } from '../utils/api'
import { t } from '../utils/i18n'
import { formatTime } from '../utils/format'

const playerRef = ref(null)
let pendingSeek = null
let raf = 0

const hasVideo = computed(() => !!player.src)
const frameNumber = computed(() => Math.round(player.currentTime * (player.fps || 30)))

function videoEl () {
  return playerRef.value?.contentEl?.querySelector?.('video') || null
}

function seekTo (time) {
  const el = videoEl()
  if (el && player.ready) {
    const duration = player.duration || el.duration || 0
    el.currentTime = Math.max(0, Math.min(duration, time))
    player.currentTime = el.currentTime
  } else {
    pendingSeek = time
  }
}

function stepFrame (delta) {
  const el = videoEl()
  if (!el) return
  el.pause()
  el.currentTime = Math.max(0, el.currentTime + delta / (player.fps || 30))
  player.currentTime = el.currentTime
}

function togglePlay () {
  const el = videoEl()
  if (!el) return
  if (el.paused) el.play().catch(() => {})
  else el.pause()
}

watch(() => player.seekNonce, () => seekTo(player.seekTarget))
watch(() => player.stepNonce, () => stepFrame(player.stepDelta))
watch(() => player.playNonce, () => togglePlay())
watch(() => player.src, () => {
  player.ready = false
  pendingSeek = null
})

function onLoadedMetadata (e) {
  player.ready = true
  if (e?.duration) player.duration = e.duration
  if (pendingSeek != null) {
    seekTo(pendingSeek)
    pendingSeek = null
  }
}

function onTimeUpdate (time) {
  if (typeof time === 'number') player.currentTime = time
}

function reveal () {
  if (currentVideo.value?.path) api.revealPath(currentVideo.value.path)
}

function startRaf () {
  const tick = () => {
    const el = videoEl()
    if (el) {
      player.currentTime = el.currentTime
      player.playing = !el.paused
      if (el.duration && Number.isFinite(el.duration)) player.duration = el.duration
    }
    raf = requestAnimationFrame(tick)
  }
  raf = requestAnimationFrame(tick)
}

onMounted(startRaf)
onBeforeUnmount(() => cancelAnimationFrame(raf))
</script>

<template>
  <div class="video-stage">
    <template v-if="hasVideo">
      <div class="stage-toolbar">
        <FluentButton size="sm" variant="subtle" icon-only :title="t('player.stepBack')" @click="requestStep(-1)">
          <FluentIcon icon="fluent:previous-frame-24-regular" width="16" height="16" />
        </FluentButton>
        <FluentButton size="sm" variant="subtle" icon-only :title="player.playing ? 'Pause' : 'Play'" @click="requestTogglePlay">
          <FluentIcon :icon="player.playing ? 'fluent:pause-24-filled' : 'fluent:play-24-filled'" width="16" height="16" />
        </FluentButton>
        <FluentButton size="sm" variant="subtle" icon-only :title="t('player.stepForward')" @click="requestStep(1)">
          <FluentIcon icon="fluent:next-frame-24-regular" width="16" height="16" />
        </FluentButton>
        <span class="time mono">{{ formatTime(player.currentTime) }}</span>
        <span class="muted mono">/ {{ formatTime(player.duration) }}</span>
        <span class="frame mono">#{{ frameNumber }}</span>
        <span class="spacer" />
        <span v-if="player.proxyState === 'ready'" class="proxy-chip">{{ t('player.proxy') }}</span>
        <FluentButton size="sm" variant="subtle" :title="t('action.reveal')" @click="reveal">
          <FluentIcon icon="fluent:folder-open-24-regular" width="15" height="15" />
        </FluentButton>
      </div>
      <div class="player-host">
        <FluentMediaPlayer
          ref="playerRef"
          :src="player.src"
          type="video"
          fit="contain"
          :show-loop="true"
          :show-playback-rate="true"
          :show-picture-in-picture="true"
          :show-minimize="false"
          @loadedmetadata="onLoadedMetadata"
          @timeupdate="onTimeUpdate"
        />
      </div>
    </template>

    <div v-else-if="player.proxyState === 'generating'" class="empty">
      <FluentProgressRing :size="36" />
      <span>{{ t('player.proxy.generating') }} {{ Math.round(player.proxyProgress * 100) }}%</span>
    </div>

    <div v-else-if="player.proxyState === 'failed'" class="empty">
      <FluentInfoBar
        severity="error"
        title="Preview failed"
        :message="t('player.proxy.failed')"
        :closable="false"
      />
    </div>

    <div v-else class="empty">
      <FluentIcon icon="fluent:video-24-regular" width="40" height="40" />
      <span class="empty-title">{{ currentVideo ? t('status.pending') : t('empty.title') }}</span>
      <span class="empty-desc">{{ currentVideo ? currentVideo.name : t('empty.desc') }}</span>
    </div>
  </div>
</template>

<style scoped>
.video-stage {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
}
.stage-toolbar {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 6px 8px;
}
.time {
  font-size: 13px;
  font-weight: 600;
}
.muted {
  color: var(--text-muted, #888);
}
.frame {
  color: var(--text-secondary, #555);
  font-size: 11px;
}
.spacer {
  flex: 1;
}
.proxy-chip {
  font-size: 11px;
  padding: 1px 6px;
  border-radius: 4px;
  background: var(--accent-50, rgba(234, 94, 193, 0.1));
  color: var(--accent, #ea5ec1);
}
.player-host {
  flex: 1;
  min-height: 0;
  display: flex;
}
.player-host :deep(.fluent-media-player),
.player-host :deep(.media-container) {
  flex: 1;
  min-height: 0;
  width: 100%;
}
.empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  color: var(--text-muted, #888);
  text-align: center;
}
.empty-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-secondary, #555);
}
.empty-desc {
  max-width: 460px;
  line-height: 1.6;
  word-break: break-all;
}
</style>
