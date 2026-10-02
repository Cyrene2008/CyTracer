// CyTracer 前端 API 封装（Tauri invoke 单通道）
// 浏览器 dev 模式下 invoke 返回 null，UI 仍可渲染空态。
const isTauri = typeof window !== 'undefined' && (
  '__TAURI_INTERNALS__' in window || '__TAURI__' in window)

export async function invoke (cmd, args) {
  if (!isTauri) return null
  const { invoke: tauriInvoke } = await import('@tauri-apps/api/core')
  return tauriInvoke(cmd, args)
}

async function invokeWithProgress (cmd, args, onProgress) {
  if (!isTauri) return null
  const { invoke: tauriInvoke, Channel } = await import('@tauri-apps/api/core')
  const channel = new Channel()
  channel.onmessage = (msg) => onProgress?.(msg)
  return tauriInvoke(cmd, { ...args, onProgress: channel })
}

export const api = {
  appVersion: () => invoke('app_version'),
  appStatus: () => invoke('app_status'),

  settingsGet: () => invoke('settings_get'),
  settingsPut: (payload) => invoke('settings_put', { payload }),

  probeVideo: (path) => invoke('probe_video', { path }),
  analyzeStart: (payload, onProgress) => invokeWithProgress('analyze_start', { payload }, onProgress),
  analyzeStatus: () => invoke('analyze_status'),
  analyzeCancel: () => invoke('analyze_cancel'),
  analysisResult: (path, params) => invoke('analysis_result', { path, params }),
  retreshold: (path, params) => invoke('events_retreshold', { path, params }),
  proxyEnsure: (path, onProgress, force = false) => invokeWithProgress('proxy_ensure', { path, force }, onProgress),
  generateSamples: () => invoke('generate_samples'),

  markersLoad: (path) => invoke('markers_load', { path }),
  markersSave: (payload) => invoke('markers_save', { payload }),
  exportMarkers: (payload) => invoke('export_markers', { payload }),
  exportContactSheet: (payload) => invoke('export_contact_sheet', { payload }),
  exportClips: (payload) => invoke('export_clips', { payload }),

  revealPath: (path) => invoke('reveal_path', { path }),
  clearCache: () => invoke('clear_cache'),
  cacheStats: () => invoke('cache_stats')
}

export const tauri = { isTauri, invoke }

export async function pickVideos () {
  if (!isTauri) return []
  const { open } = await import('@tauri-apps/plugin-dialog')
  const selected = await open({
    multiple: true,
    title: '选择视频文件',
    filters: [{
      name: '视频',
      extensions: ['mp4', 'mov', 'mkv', 'avi', 'wmv', 'flv', 'webm', 'm4v', 'mpg', 'mpeg', 'ts', 'm2ts', '3gp', 'vob', 'rmvb']
    }]
  })
  if (!selected) return []
  return Array.isArray(selected) ? selected : [selected]
}

export async function pickExportFile (defaultPath, filters) {
  if (!isTauri) return null
  const { save } = await import('@tauri-apps/plugin-dialog')
  return save({ defaultPath, filters })
}

export async function pickDir () {
  if (!isTauri) return null
  const { open } = await import('@tauri-apps/plugin-dialog')
  return open({ directory: true })
}

export async function mediaUrl (path) {
  if (!path) return ''
  if (!isTauri) return path
  const { convertFileSrc } = await import('@tauri-apps/api/core')
  return convertFileSrc(path)
}

export async function onDragDrop (handler) {
  if (!isTauri) return () => {}
  const { getCurrentWebview } = await import('@tauri-apps/api/webview')
  const unlisten = await getCurrentWebview().onDragDropEvent((event) => {
    handler(event.payload)
  })
  return unlisten
}
