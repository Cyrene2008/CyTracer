export function formatTime (seconds, { ms = true, padHours = true } = {}) {
  if (!Number.isFinite(seconds) || seconds < 0) seconds = 0
  const totalMs = Math.round(seconds * 1000)
  const h = Math.floor(totalMs / 3600000)
  const m = Math.floor((totalMs % 3600000) / 60000)
  const s = Math.floor((totalMs % 60000) / 1000)
  const milli = totalMs % 1000
  const parts = []
  if (h > 0 || padHours) parts.push(String(h).padStart(2, '0'))
  parts.push(String(m).padStart(2, '0'))
  parts.push(String(s).padStart(2, '0'))
  let text = parts.join(':')
  if (ms) text += `.${String(milli).padStart(3, '0')}`
  return text
}

export function formatShortTime (seconds) {
  if (!Number.isFinite(seconds) || seconds < 0) seconds = 0
  const h = Math.floor(seconds / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  const s = Math.floor(seconds % 60)
  if (h > 0) return `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`
  return `${m}:${String(s).padStart(2, '0')}`
}

export function formatDuration (seconds) {
  if (!Number.isFinite(seconds) || seconds <= 0) return '0s'
  const h = Math.floor(seconds / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  const s = Math.round(seconds % 60)
  if (h > 0) return `${h}h ${m}m`
  if (m > 0) return `${m}m ${s}s`
  return `${s}s`
}

export function formatBytes (bytes) {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)))
  return `${(bytes / Math.pow(1024, i)).toFixed(i === 0 ? 0 : 1)} ${units[i]}`
}

export function parseTime (text) {
  if (typeof text === 'number') return text
  if (!text) return NaN
  const trimmed = String(text).trim()
  const parts = trimmed.split(':')
  if (parts.length < 2 || parts.length > 3) return NaN
  const nums = parts.map((p) => Number(p.replace(',', '.')))
  if (nums.some((n) => !Number.isFinite(n))) return NaN
  if (nums.length === 3) return nums[0] * 3600 + nums[1] * 60 + nums[2]
  return nums[0] * 60 + nums[1]
}
