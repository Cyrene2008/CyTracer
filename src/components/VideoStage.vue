<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { currentVideo } from '../stores/library'
import { player, requestTogglePlay, requestStep, retryWithProxy } from '../stores/player'
import { api } from '../utils/api'
import { t } from '../utils/i18n'
import { formatTime } from '../utils/format'

const playerRef = ref(null)
let pendingSeek = null
let raf = 0
let popoverObserver = null
let attachedEl = null
let retriedKey = ''

// 组件库把倍速 / 音量弹层 Teleport 到 body；HTML 全屏下 body 上其他子树不可见，
// 因此把弹层搬进全屏元素，保证全屏时仍可操作。
function relocatePopovers () {
  const fullscreen = document.fullscreenElement
  if (!fullscreen) return
  document.querySelectorAll('body > .control-popover').forEach((node) => {
    if (!fullscreen.contains(node)) fullscreen.appendChild(node)
  })
}

function startPopoverBridge () {
  popoverObserver = new MutationObserver((mutations) => {
    for (const mutation of mutations) {
      for (const node of mutation.addedNodes) {
        if (node.nodeType === 1 && node.classList?.contains('control-popover') && node.parentElement === document.body) {
          relocatePopovers()
        }
      }
    }
  })
  popoverObserver.observe(document.body, { childList: true })
  document.addEventListener('fullscreenchange', relocatePopovers)
}

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

watch(() => player.videoId, () => {
  retriedKey = ''
})

function ensureVideoListeners () {
  const el = videoEl()
  if (el && el !== attachedEl) {
    attachedEl?.removeEventListener('error', onVideoError)
    attachedEl = el
    el.addEventListener('error', onVideoError)
  }
}

async function onVideoError () {
  const video = currentVideo.value
  if (!video) return
  if (player.proxyState === 'direct') {
    const key = `${video.id}:${video.path}`
    if (retriedKey === key) {
      player.proxyState = 'failed'
      player.proxyError = 'direct playback failed'
      return
    }
    retriedKey = key
    await retryWithProxy(video)
  } else if (player.proxyState === 'ready') {
    player.proxyState = 'failed'
    player.proxyError = 'proxy playback failed'
  }
}

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
    ensureVideoListeners()
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

onMounted(() => {
  startRaf()
  startPopoverBridge()
})

onBeforeUnmount(() => {
  cancelAnimationFrame(raf)
  popoverObserver?.disconnect()
  document.removeEventListener('fullscreenchange', relocatePopovers)
})
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
          :show-picture-in-picture="false"
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
      <span v-if="player.proxyError" class="err-detail">{{ player.proxyError }}</span>
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
  background: #000;
  border-radius: var(--radius-md, 8px);
  overflow: hidden;
}
.player-host :deep(.fluent-media-player) {
  flex: 1;
  min-height: 0;
  width: 100%;
  height: 100%;
}
.player-host :deep(.media-container) {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}
.player-host :deep(.media-container video) {
  width: 100%;
  height: 100%;
  object-fit: contain;
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
.err-detail {
  max-width: 540px;
  font-size: 11px;
  color: var(--text-muted, #888);
  word-break: break-all;
}
</style>
