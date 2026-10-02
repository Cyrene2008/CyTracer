import { reactive } from 'vue'
import { api, mediaUrl } from '../utils/api'

let loadToken = 0

export const player = reactive({
  videoId: '',
  src: '',
  playback: 'direct',
  proxyState: 'idle', // idle | direct | generating | ready | failed
  proxyProgress: 0,
  playing: false,
  ready: false,
  currentTime: 0,
  duration: 0,
  fps: 30,
  seekNonce: 0,
  seekTarget: 0,
  stepNonce: 0,
  stepDelta: 0,
  playNonce: 0
})

export function resetPlayer () {
  player.videoId = ''
  player.src = ''
  player.proxyState = 'idle'
  player.proxyProgress = 0
  player.playing = false
  player.ready = false
  player.currentTime = 0
  player.duration = 0
}

export async function loadVideo (video) {
  const token = ++loadToken
  resetPlayer()
  player.videoId = video.id
  player.fps = video.info?.fps || 30
  player.duration = video.info?.duration || 0

  if (!video?.path) return

  if (video.info?.playback === 'direct') {
    player.proxyState = 'direct'
    player.src = await mediaUrl(video.path)
    if (token !== loadToken) return
    return
  }

  player.proxyState = 'generating'
  player.proxyProgress = 0
  try {
    const result = await api.proxyEnsure(video.path, (msg) => {
      if (msg && typeof msg.progress === 'number') player.proxyProgress = msg.progress
    })
    if (token !== loadToken) return
    if (result && result.path) {
      player.src = await mediaUrl(result.path)
      player.proxyState = 'ready'
    } else if (result === null) {
      // 浏览器 dev：直接尝试源文件
      player.src = await mediaUrl(video.path)
      player.proxyState = 'ready'
    }
  } catch (err) {
    if (token !== loadToken) return
    console.error('[proxy]', err)
    player.proxyState = 'failed'
  }
}

export function requestSeek (time) {
  player.seekTarget = Math.max(0, time)
  player.seekNonce += 1
}

export function requestStep (deltaFrames) {
  player.stepDelta = deltaFrames
  player.stepNonce += 1
}

export function requestTogglePlay () {
  player.playNonce += 1
}
