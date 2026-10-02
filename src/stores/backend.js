import { reactive } from 'vue'
import { api } from '../utils/api'

export const backendState = reactive({
  checked: false,
  ready: false,
  ffmpeg: false,
  version: '',
  error: '',
  ffmpegVersion: ''
})

let timer = null
let interval = 2000

async function refresh () {
  try {
    const status = await api.appStatus()
    if (status) {
      backendState.checked = true
      backendState.ready = !!status.ready
      backendState.ffmpeg = !!status.ffmpeg
      backendState.version = status.version || backendState.version
      backendState.ffmpegVersion = status.ffmpegVersion || ''
      backendState.error = status.error || ''
      if (backendState.ready) interval = 30000
    } else {
      // 浏览器 dev：视为可用
      backendState.checked = true
      backendState.ready = false
    }
  } catch (err) {
    backendState.checked = true
    backendState.ready = false
    backendState.error = String(err?.message ?? err)
  }
}

function loop () {
  clearTimeout(timer)
  timer = setTimeout(async () => {
    await refresh()
    loop()
  }, interval)
}

export function startBackendPoll () {
  if (timer) return
  refresh().then(loop)
}

export function stopBackendPoll () {
  clearTimeout(timer)
  timer = null
}
