import { reactive } from 'vue'
import { api } from '../utils/api'

const defaults = {
  lang: 'zh',
  theme: 'light',
  palette: 'peach',
  exportDir: '',
  analyze: {
    preset: 'custom',
    speed: 'standard',
    fps: 8,
    sensitivity: 50,
    minDuration: 0.4,
    mergeGap: 0.3,
    compensateCamera: true,
    markSceneCuts: true
  }
}

export const settings = reactive(structuredClone(defaults))

const listeners = new Set()
let saveTimer = null
let loaded = false

export function onSettingsChange (fn) {
  listeners.add(fn)
  return () => listeners.delete(fn)
}

function notify () {
  for (const fn of listeners) {
    try { fn(settings) } catch (err) { console.error(err) }
  }
}

export async function loadSettingsFromBackend () {
  if (loaded) return
  loaded = true
  try {
    const remote = await api.settingsGet()
    if (remote && typeof remote === 'object') {
      Object.assign(settings, { ...structuredClone(defaults), ...remote })
      settings.analyze = { ...defaults.analyze, ...(remote.analyze || {}) }
      notify()
    }
  } catch {
    // 浏览器 dev 或引擎未就绪时使用默认值
  }
}

export function updateSettings (patch) {
  Object.assign(settings, patch)
  notify()
  scheduleSave()
}

export function updateAnalyze (patch) {
  Object.assign(settings.analyze, patch)
  notify()
  scheduleSave()
}

function scheduleSave () {
  if (saveTimer) clearTimeout(saveTimer)
  saveTimer = setTimeout(() => {
    saveTimer = null
    api.settingsPut(structuredClone(settings)).catch(() => {})
  }, 400)
}
