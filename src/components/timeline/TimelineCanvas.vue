<script setup>
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { addMarkerAt, currentVideo, deleteEvent, library, mergeWithNext, selectEvent, updateEvent } from '../../stores/library'
import { player, requestSeek } from '../../stores/player'
import { t } from '../../utils/i18n'
import { formatTime } from '../../utils/format'

const kindLabel = (event) => {
  if (event.kind === 'cut') return t('marker.type.cut')
  if (event.kind === 'manual') return t('marker.type.manual')
  return t('marker.type.motion')
}

const RULER_H = 22
const CURVE_H = 54
const MARKER_H = 28
const CUTS_H = 16
const HEIGHT = RULER_H + CURVE_H + MARKER_H + CUTS_H
const EDGE_PX = 5

const canvasRef = ref(null)
const rootRef = ref(null)
const viewStart = ref(0)
const viewEnd = ref(10)
const tooltip = reactive({ show: false, x: 0, y: 0, eventId: '' })

const duration = computed(() => {
  const video = currentVideo.value
  return Math.max(player.duration || video?.duration || video?.info?.duration || 0, 0.1)
})

const events = computed(() => currentVideo.value?.events || [])

let cssW = 0
let cssH = HEIGHT
let hitList = []
let drag = null
let drawQueued = false
let ro = null

const pxPerSec = () => (viewEnd.value - viewStart.value) > 0
  ? cssW / (viewEnd.value - viewStart.value)
  : 1

function timeAt (x) {
  return viewStart.value + x / pxPerSec()
}

function xAt (t) {
  return (t - viewStart.value) * pxPerSec()
}

function requestDraw () {
  if (drawQueued) return
  drawQueued = true
  requestAnimationFrame(() => {
    drawQueued = false
    draw()
  })
}

function roundRect (ctx, x, y, w, h, r) {
  const radius = Math.min(r, h / 2, Math.max(w / 2, 0.1))
  ctx.beginPath()
  ctx.moveTo(x + radius, y)
  ctx.arcTo(x + w, y, x + w, y + h, radius)
  ctx.arcTo(x + w, y + h, x, y + h, radius)
  ctx.arcTo(x, y + h, x, y, radius)
  ctx.arcTo(x, y, x + w, y, radius)
  ctx.closePath()
}

function levelColor (event, accent) {
  if (event.kind === 'cut') return '#8a8886'
  if (event.kind === 'manual') return '#107c10'
  const palette = {
    2: accent,
    1: 'rgba(234, 94, 193, 0.75)',
    0: 'rgba(234, 94, 193, 0.5)'
  }
  return palette[event.level] ?? palette[0]
}

function cssVar (name, fallback) {
  if (typeof window === 'undefined') return fallback
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim()
  return value || fallback
}

function tickStep (secondsPerPixel) {
  const targets = [0.1, 0.2, 0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300, 600, 1800, 3600]
  const minPx = 70
  for (const step of targets) {
    if (step / secondsPerPixel >= minPx) return step
  }
  return targets[targets.length - 1]
}

function tickLabel (time, step) {
  const total = Math.max(0, time)
  const h = Math.floor(total / 3600)
  const m = Math.floor((total % 3600) / 60)
  const s = total % 60
  if (step < 1) {
    return `${h > 0 ? `${h}:` : ''}${String(m).padStart(h > 0 ? 2 : 1, '0')}:${s.toFixed(1).padStart(4, '0')}`
  }
  return `${h > 0 ? `${h}:` : ''}${String(m).padStart(h > 0 ? 2 : 1, '0')}:${String(Math.floor(s)).padStart(2, '0')}`
}

function draw () {
  const canvas = canvasRef.value
  if (!canvas) return
  const dpr = window.devicePixelRatio || 1
  cssW = canvas.clientWidth
  cssH = HEIGHT
  if (canvas.width !== Math.round(cssW * dpr) || canvas.height !== Math.round(cssH * dpr)) {
    canvas.width = Math.round(cssW * dpr)
    canvas.height = Math.round(cssH * dpr)
  }
  const ctx = canvas.getContext('2d')
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  ctx.clearRect(0, 0, cssW, cssH)

  const accent = cssVar('--accent', '#ea5ec1')
  const border = cssVar('--border-subtle', 'rgba(0,0,0,0.08)')
  const muted = cssVar('--text-muted', '#888')
  const bgCard = cssVar('--bg-card-solid', 'rgba(255,255,255,0.6)')

  const laneY = { ruler: 0, curve: RULER_H, markers: RULER_H + CURVE_H, cuts: RULER_H + CURVE_H + MARKER_H }
  const laneH = { ruler: RULER_H, curve: CURVE_H, markers: MARKER_H, cuts: CUTS_H }

  ctx.fillStyle = bgCard
  ctx.fillRect(0, 0, cssW, cssH)

  // 网格
  const step = tickStep((viewEnd.value - viewStart.value) / Math.max(cssW, 1))
  ctx.strokeStyle = border
  ctx.lineWidth = 1
  ctx.fillStyle = muted
  ctx.font = '10px "MiSans", "Segoe UI", sans-serif'
  ctx.textBaseline = 'middle'
  const first = Math.floor(viewStart.value / step) * step
  for (let t = first; t <= viewEnd.value + step; t += step) {
    const x = Math.round(xAt(t)) + 0.5
    if (x < -40 || x > cssW + 40) continue
    ctx.beginPath()
    ctx.moveTo(x, RULER_H)
    ctx.lineTo(x, HEIGHT - CUTS_H)
    ctx.stroke()
    ctx.fillText(tickLabel(t, step), x + 4, RULER_H / 2 + 1)
  }

  // 车道分隔线
  ctx.strokeStyle = border
  for (const y of [laneY.curve, laneY.markers, laneY.cuts]) {
    ctx.beginPath()
    ctx.moveTo(0, y + 0.5)
    ctx.lineTo(cssW, y + 0.5)
    ctx.stroke()
  }

  // 运动强度曲线
  const video = currentVideo.value
  const curve = video?.curve || []
  const buckets = Math.max(video?.curveBuckets || curve.length || 1, 1)
  const dur = duration.value
  if (curve.length) {
    const visible = []
    for (let x = 0; x <= cssW; x += 2) {
      const t = timeAt(x)
      if (t < 0 || t > dur) continue
      const idx = Math.min(buckets - 1, Math.max(0, Math.floor((t / dur) * buckets)))
      visible.push(curve[Math.min(curve.length - 1, idx)] || 0)
    }
    const maxV = Math.max(0.02, ...visible)
    const baseY = laneY.curve + laneH.curve
    ctx.beginPath()
    ctx.moveTo(0, baseY)
    for (let x = 0; x <= cssW; x += 2) {
      const t = timeAt(x)
      const idx = Math.min(buckets - 1, Math.max(0, Math.floor((t / dur) * buckets)))
      const value = t < 0 || t > dur ? 0 : (curve[Math.min(curve.length - 1, idx)] || 0)
      const h = Math.min(1, value / maxV) * (laneH.curve - 8)
      ctx.lineTo(x, baseY - h)
    }
    ctx.lineTo(cssW, baseY)
    ctx.closePath()
    const gradient = ctx.createLinearGradient(0, laneY.curve, 0, baseY)
    gradient.addColorStop(0, 'rgba(234, 94, 193, 0.55)')
    gradient.addColorStop(1, 'rgba(234, 94, 193, 0.08)')
    ctx.fillStyle = gradient
    ctx.fill()
    ctx.strokeStyle = accent
    ctx.lineWidth = 1
    ctx.stroke()

    // 噪声底参考线
    if (video?.noiseFloor > 0) {
      const y = baseY - Math.min(1, video.noiseFloor / maxV) * (laneH.curve - 8)
      ctx.setLineDash([4, 4])
      ctx.strokeStyle = muted
      ctx.beginPath()
      ctx.moveTo(0, y)
      ctx.lineTo(cssW, y)
      ctx.stroke()
      ctx.setLineDash([])
    }
  }

  // 标记
  hitList = []
  const markerTop = laneY.markers + 4
  const markerH = laneH.markers - 8
  for (const event of events.value) {
    const x0 = xAt(event.start)
    const x1 = xAt(event.end)
    if (x1 < -20 || x0 > cssW + 20) continue
    const w = Math.max(3, x1 - x0)
    const selected = event.id === library.selectedEventId
    const color = levelColor(event, accent)

    ctx.globalAlpha = selected ? 1 : 0.82
    roundRect(ctx, x0, markerTop, w, markerH, 5)
    ctx.fillStyle = color
    ctx.fill()
    if (selected) {
      ctx.strokeStyle = cssVar('--text-primary', '#222')
      ctx.lineWidth = 1.5
      ctx.stroke()
      ctx.fillStyle = cssVar('--text-primary', '#222')
      ctx.fillRect(x0 - 1, markerTop - 3, 2, markerH + 6)
      ctx.fillRect(x1 - 1, markerTop - 3, 2, markerH + 6)
    }
    ctx.globalAlpha = 1

    if (w > 34) {
      ctx.save()
      ctx.beginPath()
      ctx.rect(x0, markerTop, w, markerH)
      ctx.clip()
      ctx.fillStyle = 'rgba(255,255,255,0.95)'
      ctx.font = '10px "MiSans", "Segoe UI", sans-serif'
      ctx.fillText(event.label || '', x0 + 5, markerTop + markerH / 2 + 0.5)
      ctx.restore()
    }
    hitList.push({ event, x0, x1, lane: 'markers' })
  }

  // 镜头切换刻度
  const cutTop = laneY.cuts + 3
  for (const event of events.value) {
    if (event.kind !== 'cut') continue
    const x = xAt(event.peak)
    if (x < -4 || x > cssW + 4) continue
    ctx.fillStyle = '#8a8886'
    ctx.fillRect(x - 0.5, cutTop, 1.5, laneH.cuts - 6)
    hitList.push({ event, x0: x - 3, x1: x + 3, lane: 'cuts' })
  }

  // 播放头
  if (player.ready || player.currentTime > 0) {
    const x = Math.round(xAt(player.currentTime)) + 0.5
    if (x >= 0 && x <= cssW) {
      ctx.strokeStyle = cssVar('--text-primary', '#222')
      ctx.lineWidth = 1
      ctx.beginPath()
      ctx.moveTo(x, RULER_H)
      ctx.lineTo(x, HEIGHT - CUTS_H)
      ctx.stroke()
      ctx.fillStyle = cssVar('--text-primary', '#222')
      roundRect(ctx, x - 22, 1, 44, RULER_H - 3, 4)
      ctx.fill()
      ctx.fillStyle = cssVar('--bg-card-solid', '#fff')
      ctx.font = '10px "Cascadia Mono", monospace'
      ctx.textAlign = 'center'
      ctx.fillText(formatTime(player.currentTime, { padHours: false }).slice(0, 8), x, RULER_H / 2)
      ctx.textAlign = 'left'
    }
  }
}

function hitTest (x, y) {
  if (y < RULER_H) return { lane: 'ruler' }
  for (let i = hitList.length - 1; i >= 0; i -= 1) {
    const hit = hitList[i]
    if (x >= hit.x0 - EDGE_PX && x <= hit.x1 + EDGE_PX) {
      const nearStart = Math.abs(x - hit.x0) <= EDGE_PX
      const nearEnd = Math.abs(x - hit.x1) <= EDGE_PX
      return { ...hit, nearStart, nearEnd }
    }
  }
  return { lane: 'empty' }
}

function onPointerDown (e) {
  const x = e.offsetX
  const y = e.offsetY
  const hit = hitTest(x, y)
  canvasRef.value.setPointerCapture(e.pointerId)

  if (e.button === 1 || e.shiftKey) {
    drag = { mode: 'pan', startX: x, viewStart: viewStart.value, viewEnd: viewEnd.value }
    return
  }
  if (hit.event) {
    selectEvent(hit.event.id)
    if (hit.nearStart) {
      drag = { mode: 'resize-start', event: hit.event, origStart: hit.event.start, origEnd: hit.event.end }
    } else if (hit.nearEnd) {
      drag = { mode: 'resize-end', event: hit.event, origStart: hit.event.start, origEnd: hit.event.end }
    } else {
      drag = { mode: 'move', event: hit.event, startX: x, origStart: hit.event.start, origEnd: hit.event.end }
    }
    return
  }
  drag = { mode: 'playhead' }
  requestSeek(timeAt(x))
}

function onPointerMove (e) {
  const x = e.offsetX
  const y = e.offsetY

  if (drag) {
    if (drag.mode === 'pan') {
      const dt = (drag.startX - x) / pxPerSec()
      viewStart.value = drag.viewStart + dt
      viewEnd.value = drag.viewEnd + dt
      clampView()
      requestDraw()
    } else if (drag.mode === 'playhead') {
      requestSeek(timeAt(x))
    } else if (drag.event) {
      const dt = (x - (drag.startX ?? x)) / pxPerSec()
      const minGap = 0.05
      if (drag.mode === 'move') {
        const span = drag.origEnd - drag.origStart
        let start = drag.origStart + dt
        start = Math.max(0, Math.min(duration.value - span, start))
        updateEvent(drag.event.id, { start, end: start + span, peak: Math.max(start, Math.min(start + span, drag.event.peak)) })
      } else if (drag.mode === 'resize-start') {
        const start = Math.max(0, Math.min(drag.origEnd - minGap, drag.origStart + dt))
        updateEvent(drag.event.id, { start })
      } else if (drag.mode === 'resize-end') {
        const end = Math.min(duration.value, Math.max(drag.origStart + minGap, drag.origEnd + dt))
        updateEvent(drag.event.id, { end })
      }
      requestDraw()
    }
    return
  }

  const hit = hitTest(x, y)
  const canvas = canvasRef.value
  if (hit.event) {
    tooltip.show = true
    tooltip.x = x
    tooltip.y = y
    tooltip.eventId = hit.event.id
    canvas.style.cursor = hit.nearStart || hit.nearEnd ? 'ew-resize' : 'pointer'
  } else {
    tooltip.show = false
    canvas.style.cursor = 'crosshair'
  }
}

function onPointerUp (e) {
  if (drag?.event) {
    // 拖动结束统一保存一次（内部为防抖保存，触发即可）
    updateEvent(drag.event.id, {})
  }
  drag = null
  try { canvasRef.value.releasePointerCapture(e.pointerId) } catch { /* ignore */ }
}

function onWheel (e) {
  e.preventDefault()
  const x = e.offsetX
  if (e.ctrlKey || e.metaKey) {
    const factor = e.deltaY > 0 ? 1.25 : 0.8
    zoomAt(x, factor)
  } else {
    const delta = (e.deltaY + e.deltaX) / pxPerSec()
    viewStart.value += delta
    viewEnd.value += delta
    clampView()
    requestDraw()
  }
}

function zoomAt (x, factor) {
  const anchor = timeAt(x)
  let span = (viewEnd.value - viewStart.value) * factor
  span = Math.max(0.2, Math.min(duration.value, span))
  const ratio = x / Math.max(cssW, 1)
  viewStart.value = anchor - span * ratio
  viewEnd.value = viewStart.value + span
  clampView()
  requestDraw()
}

function clampView () {
  const span = viewEnd.value - viewStart.value
  if (viewStart.value < 0) {
    viewStart.value = 0
    viewEnd.value = span
  }
  if (viewEnd.value > duration.value) {
    viewEnd.value = duration.value
    viewStart.value = Math.max(0, viewEnd.value - span)
  }
}

function onDblClick (e) {
  const hit = hitTest(e.offsetX, e.offsetY)
  if (hit.event) return
  addMarkerAt(timeAt(e.offsetX), Math.min(1, duration.value / 10))
  requestDraw()
}

function fit () {
  viewStart.value = 0
  viewEnd.value = duration.value
  requestDraw()
}

function zoom (factor) {
  zoomAt(cssW / 2, factor)
}

function onKeyDown (e) {
  if (e.target !== document.body && e.target?.tagName !== 'CANVAS') return
  if (e.key === 'Delete' || e.key === 'Backspace') {
    if (library.selectedEventId) {
      deleteEvent(library.selectedEventId)
      requestDraw()
    }
  } else if (e.key === 'm' || e.key === 'M') {
    addMarkerAt(player.currentTime, Math.min(1, duration.value / 10))
    requestDraw()
  } else if (e.key === 'ArrowLeft') {
    requestSeek(player.currentTime - (e.shiftKey ? 1 : 1 / (player.fps || 30)))
  } else if (e.key === 'ArrowRight') {
    requestSeek(player.currentTime + (e.shiftKey ? 1 : 1 / (player.fps || 30)))
  }
}

const tooltipEvent = computed(() => events.value.find((e) => e.id === tooltip.eventId))

watch(
  () => [currentVideo.value?.id, player.currentTime, library.selectedEventId, viewStart.value, viewEnd.value, currentVideo.value?.events.length],
  requestDraw
)
watch(() => currentVideo.value?.id, () => {
  fit()
})

onMounted(() => {
  fit()
  ro = new ResizeObserver(requestDraw)
  if (rootRef.value) ro.observe(rootRef.value)
  window.addEventListener('keydown', onKeyDown)
  requestDraw()
})

onBeforeUnmount(() => {
  if (ro) ro.disconnect()
  window.removeEventListener('keydown', onKeyDown)
})

defineExpose({ fit, zoom, requestDraw })
</script>

<template>
  <div ref="rootRef" class="timeline-root">
    <canvas
      ref="canvasRef"
      class="timeline-canvas"
      :style="{ height: HEIGHT + 'px' }"
      @pointerdown="onPointerDown"
      @pointermove="onPointerMove"
      @pointerup="onPointerUp"
      @pointercancel="onPointerUp"
      @wheel="onWheel"
      @dblclick="onDblClick"
    />
    <div
      v-if="tooltip.show && tooltipEvent"
      class="marker-tooltip"
      :style="{ left: Math.min(tooltip.x + 12, cssW - 210) + 'px', top: (tooltip.y + 16) + 'px' }"
    >
      <div class="tip-row">
        <strong>{{ kindLabel(tooltipEvent) }}</strong>
        <span class="mono">{{ formatTime(tooltipEvent.start) }} → {{ formatTime(tooltipEvent.end) }}</span>
      </div>
      <div class="tip-row muted">
        <span>{{ (tooltipEvent.end - tooltipEvent.start).toFixed(2) }}s</span>
        <span class="mono">score {{ (tooltipEvent.score || 0).toFixed(3) }}</span>
      </div>
      <div v-if="tooltipEvent.label" class="tip-row">{{ tooltipEvent.label }}</div>
    </div>
  </div>
</template>

<style scoped>
.timeline-root {
  position: relative;
  width: 100%;
  min-height: 120px;
}
.timeline-canvas {
  display: block;
  width: 100%;
  touch-action: none;
}
.marker-tooltip {
  position: absolute;
  z-index: 30;
  pointer-events: none;
  background: var(--bg-card-solid, #fff);
  border: 1px solid var(--border-subtle, rgba(0, 0, 0, 0.1));
  border-radius: 6px;
  box-shadow: var(--shadow-8, 0 4px 12px rgba(0, 0, 0, 0.14));
  padding: 6px 8px;
  font-size: 11px;
  max-width: 220px;
}
.tip-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}
.tip-row + .tip-row {
  margin-top: 2px;
}
.muted {
  color: var(--text-muted, #888);
}
</style>
