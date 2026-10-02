import { computed, reactive } from 'vue'
import { api, pickDir, pickExportFile, pickVideos } from '../utils/api'
import { settings } from './settings'
import { pushToast } from '../components/ToastHost.vue'
import { t } from '../utils/i18n'
import { loadVideo, player } from './player'

const VIDEO_EXTS = ['mp4', 'mov', 'mkv', 'avi', 'wmv', 'flv', 'webm', 'm4v', 'mpg', 'mpeg', 'ts', 'm2ts', '3gp', 'vob', 'rmvb']

let uid = 0
let saveTimer = null

export const library = reactive({
  videos: [],
  currentId: '',
  selectedEventId: '',
  importing: false,
  queueRunning: false
})

export const currentVideo = computed(
  () => library.videos.find((v) => v.id === library.currentId) || null
)

export const selectedEvent = computed(() => {
  const video = currentVideo.value
  if (!video) return null
  return video.events.find((e) => e.id === library.selectedEventId) || null
})

export const analyzedVideos = computed(() =>
  library.videos.filter((v) => v.status === 'done')
)

export const paramsFromSettings = () => ({
  fps: settings.analyze.fps,
  sensitivity: settings.analyze.sensitivity,
  minDuration: settings.analyze.minDuration,
  mergeGap: settings.analyze.mergeGap,
  compensateCamera: settings.analyze.compensateCamera,
  markSceneCuts: settings.analyze.markSceneCuts
})

export function isVideoFile (path) {
  const ext = String(path).split('.').pop()?.toLowerCase() || ''
  return VIDEO_EXTS.includes(ext)
}

export async function importPaths (paths) {
  const files = (paths || []).filter(isVideoFile)
  if (!files.length) return
  library.importing = true
  let added = 0
  for (const path of files) {
    if (library.videos.some((v) => v.path === path)) continue
    const entry = reactive({
      id: `v-${++uid}`,
      path,
      name: path.split(/[\\/]/).pop() || path,
      status: 'probing',
      progress: 0,
      speed: 0,
      elapsedMs: 0,
      error: '',
      info: null,
      events: [],
      curve: [],
      curveBuckets: 1,
      frameCount: 0,
      noiseFloor: 0,
      width: 0,
      height: 0,
      analyzedAt: 0,
      duration: 0
    })
    library.videos.push(entry)
    try {
      const info = await api.probeVideo(path)
      if (info) {
        entry.info = info
        entry.duration = info.duration || 0
        entry.status = 'pending'
      } else {
        // 浏览器 dev 模式
        entry.status = 'pending'
      }
      added += 1
    } catch (err) {
      entry.status = 'failed'
      entry.error = String(err?.message ?? err)
    }
  }
  library.importing = false
  if (added && !library.currentId) {
    selectVideo(library.videos.find((v) => v.status !== 'failed')?.id || '')
  }
  if (added) {
    pushToast({ title: t('toast.imported', { n: added }) })
  }
}

export async function importViaDialog () {
  const paths = await pickVideos()
  if (paths?.length) await importPaths(paths)
}

export function selectVideo (id) {
  if (library.currentId === id) return
  library.currentId = id
  library.selectedEventId = ''
  const video = library.videos.find((v) => v.id === id)
  if (video) {
    loadVideo(video)
  } else {
    player.videoId = ''
    player.src = ''
  }
}

export function removeVideo (id) {
  const index = library.videos.findIndex((v) => v.id === id)
  if (index < 0) return
  library.videos.splice(index, 1)
  if (library.currentId === id) {
    selectVideo(library.videos[Math.min(index, library.videos.length - 1)]?.id || '')
  }
}

export function clearVideos () {
  library.videos.splice(0, library.videos.length)
  selectVideo('')
}

export async function analyzeVideo (id) {
  const video = library.videos.find((v) => v.id === id)
  if (!video) return
  const params = paramsFromSettings()
  video.status = 'analyzing'
  video.progress = 0
  video.error = ''
  try {
    await new Promise((resolve, reject) => {
      let settled = false
      api
        .analyzeStart({ path: video.path, params }, (msg) => {
          if (!msg) return
          if (typeof msg.progress === 'number') video.progress = msg.progress
          if (typeof msg.speed === 'number') video.speed = msg.speed
          if (typeof msg.elapsedMs === 'number') video.elapsedMs = msg.elapsedMs
          if (msg.processed) video.processed = msg.processed
          if (msg.total) video.total = msg.total
          if (msg.stage === 'done') {
            settled = true
            resolve()
          } else if (msg.stage === 'error') {
            settled = true
            reject(new Error(msg.message || 'analysis failed'))
          } else if (msg.stage === 'canceled') {
            settled = true
            reject(Object.assign(new Error('canceled'), { canceled: true }))
          }
        })
        .then((result) => {
          // 浏览器 dev 模式：无后端，直接结束
          if (result === null && !settled) resolve()
        })
        .catch((err) => {
          if (!settled) reject(err)
        })
    })

    const result = await api.analysisResult(video.path, params)
    if (result) {
      applyResult(video, result)
    }
    video.status = 'done'
    video.progress = 1
    pushToast({ title: t('toast.analyzeDone', { name: video.name }) })
  } catch (err) {
    if (err?.canceled) {
      video.status = 'canceled'
      pushToast({ title: t('toast.analyzeCanceled') })
    } else {
      video.status = 'failed'
      video.error = String(err?.message ?? err)
      pushToast({ title: t('toast.analyzeFailed', { msg: video.error }) })
    }
  }
}

function applyResult (video, result) {
  video.events = (result.events || []).map((e) => ({ ...e }))
  video.curve = result.curve || []
  video.curveBuckets = result.curveBuckets || 1
  video.frameCount = result.frameCount || 0
  video.noiseFloor = result.noiseFloor || 0
  video.width = result.width || video.width
  video.height = result.height || video.height
  video.analyzedAt = result.analyzedAt || Date.now()
  if (result.info) video.info = result.info
  if (result.info?.duration) video.duration = result.info.duration
  if (video.info?.fps) player.fps = video.info.fps
}

export async function cancelAnalysis () {
  await api.analyzeCancel()
}

export async function analyzeAll () {
  if (library.queueRunning) return
  library.queueRunning = true
  try {
    for (const video of library.videos) {
      if (video.status === 'pending' || video.status === 'canceled' || video.status === 'failed') {
        await analyzeVideo(video.id)
        if (video.status === 'canceled') break
      }
    }
  } finally {
    library.queueRunning = false
  }
}

export async function applyParams () {
  const video = currentVideo.value
  if (!video || video.status !== 'done') return
  const params = paramsFromSettings()
  try {
    const result = await api.retreshold(video.path, params)
    if (result) {
      applyResult(video, result)
      pushToast({ title: t('params.rethreshold.done') })
    }
  } catch (err) {
    if (String(err).includes('尚未')) {
      // 帧率 / 补偿方式变化导致需要重新分析
      video.status = 'pending'
      pushToast({ title: String(err?.message ?? err) })
    } else {
      console.error(err)
    }
  }
}

function scheduleSave () {
  const video = currentVideo.value
  if (!video) return
  if (saveTimer) clearTimeout(saveTimer)
  saveTimer = setTimeout(async () => {
    saveTimer = null
    try {
      await api.markersSave({
        path: video.path,
        events: video.events.map((e) => ({ ...e })),
        updatedAt: 0
      })
    } catch (err) {
      console.error('[markers]', err)
    }
  }, 500)
}

export function selectEvent (id) {
  library.selectedEventId = id
}

export function updateEvent (id, patch) {
  const video = currentVideo.value
  if (!video) return
  const event = video.events.find((e) => e.id === id)
  if (!event) return
  Object.assign(event, patch)
  if (event.auto && !event.edited) event.edited = true
  scheduleSave()
}

export function deleteEvent (id) {
  const video = currentVideo.value
  if (!video) return
  const index = video.events.findIndex((e) => e.id === id)
  if (index < 0) return
  video.events.splice(index, 1)
  if (library.selectedEventId === id) library.selectedEventId = ''
  scheduleSave()
}

export function addMarkerAt (time, duration = 1) {
  const video = currentVideo.value
  if (!video) return
  const start = Math.max(0, time)
  const end = Math.min(video.duration || start + duration, start + duration)
  const event = reactive({
    id: `manual-${Date.now()}`,
    kind: 'manual',
    start,
    end,
    peak: start,
    score: 0,
    level: 1,
    label: '',
    note: '',
    color: '',
    auto: false,
    edited: true
  })
  video.events.push(event)
  video.events.sort((a, b) => a.start - b.start)
  library.selectedEventId = event.id
  scheduleSave()
  return event
}

export function mergeWithNext (id) {
  const video = currentVideo.value
  if (!video) return
  const index = video.events.findIndex((e) => e.id === id)
  if (index < 0 || index >= video.events.length - 1) return
  const current = video.events[index]
  const next = video.events[index + 1]
  current.end = Math.max(current.end, next.end)
  if (next.score > current.score) {
    current.score = next.score
    current.level = next.level
    current.peak = next.peak
  }
  video.events.splice(index + 1, 1)
  scheduleSave()
}

export async function exportMarkers ({ format, scope, includeCuts }) {
  const videos = scope === 'all' ? analyzedVideos.value : [currentVideo.value].filter(Boolean)
  const payloadVideos = videos
    .filter((v) => v && v.events.length)
    .map((v) => ({
      name: v.name,
      path: v.path,
      events: v.events.map((e) => ({ ...e }))
    }))
  if (!payloadVideos.length) {
    pushToast({ title: t('export.empty') })
    return null
  }
  const ext = format === 'srt' ? 'srt' : format
  const defaultName = `cytracer-markers.${ext}`
  const outPath = await pickExportFile(defaultName, [{ name: ext.toUpperCase(), extensions: [ext] }])
  if (!outPath) return null
  const result = await api.exportMarkers({
    outPath,
    format,
    includeCuts,
    videos: payloadVideos
  })
  const path = result?.path || outPath
  pushToast({ title: t('export.done', { path }) })
  return path
}

export async function exportContactSheet ({ scope, includeCuts }) {
  const videos = scope === 'all' ? analyzedVideos.value : [currentVideo.value].filter(Boolean)
  const payloadVideos = videos
    .filter((v) => v && v.events.length)
    .map((v) => ({
      name: v.name,
      path: v.path,
      events: v.events.map((e) => ({ ...e }))
    }))
  if (!payloadVideos.length) {
    pushToast({ title: t('export.empty') })
    return null
  }
  const outDir = await pickDir()
  if (!outDir) return null
  const result = await api.exportContactSheet({
    outDir,
    includeCuts,
    columns: 4,
    thumbWidth: 480,
    videos: payloadVideos
  })
  if (result) {
    pushToast({ title: t('export.sheet.done', { n: result.events, dir: outDir }) })
  }
  return result
}

export async function exportClips ({ scope, includeCuts, mode = 'reel' }) {
  const videos = scope === 'all' ? analyzedVideos.value : [currentVideo.value].filter(Boolean)
  const payloadVideos = videos
    .filter((v) => v && v.events.length)
    .map((v) => ({
      name: v.name,
      path: v.path,
      events: v.events.map((e) => ({ ...e }))
    }))
  if (!payloadVideos.length) {
    pushToast({ title: t('export.empty') })
    return null
  }
  const outDir = await pickDir()
  if (!outDir) return null
  const result = await api.exportClips({
    outDir,
    includeCuts,
    merge: mode !== 'clips',
    keepClips: mode !== 'reel',
    pad: settings.exportPad ?? 0.3,
    videos: payloadVideos
  })
  if (result) {
    if (result.reelErrors?.length) {
      pushToast({
        title: t('export.clips.reelFailed', { n: result.reelErrors.length }),
        message: result.reelErrors[0]
      })
    } else if (mode === 'reel' && result.reels?.length) {
      const names = result.reels.map((p) => String(p).split(/[\\/]/).pop())
      pushToast({
        title: t('export.reel.done', { name: names[0] }),
        message: names.length > 1 ? t('export.reel.more', { n: names.length }) : ''
      })
    } else {
      const total = (result.events || 0) + (result.reels?.length || 0)
      pushToast({ title: t('export.clips.done', { n: total, dir: outDir }) })
    }
  }
  return result
}
