// 开发期 UI 布局巡检：Playwright 打开浏览器模式的三页，截图 + 断言布局。
// 用法：bun run ui:check
import { spawn } from 'node:child_process'
import { mkdirSync, existsSync } from 'node:fs'
import { connect } from 'node:net'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium } from 'playwright'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const shotsDir = join(root, '.plan', 'screenshots')
const baseUrl = 'http://127.0.0.1:5173'
const viewport = { width: 1360, height: 860 }

function portOpen (host, port, timeout = 400) {
  return new Promise((resolvePromise) => {
    const socket = connect({ host, port })
    const done = (value) => {
      socket.destroy()
      resolvePromise(value)
    }
    socket.setTimeout(timeout)
    socket.once('connect', () => done(true))
    socket.once('timeout', () => done(false))
    socket.once('error', () => done(false))
  })
}

async function waitForServer (timeoutMs = 60000) {
  const start = Date.now()
  while (Date.now() - start < timeoutMs) {
    if (await portOpen('127.0.0.1', 5173)) return true
    await new Promise((r) => setTimeout(r, 400))
  }
  return false
}

async function ensureServer () {
  if (await portOpen('127.0.0.1', 5173)) return null
  const child = spawn('bun', ['run', 'dev'], {
    cwd: root,
    stdio: 'ignore',
    shell: true,
    detached: false
  })
  const ok = await waitForServer()
  if (!ok) {
    child.kill()
    throw new Error('Vite dev server failed to start')
  }
  return child
}

function killTree (child) {
  if (!child?.pid) return
  try {
    spawn('taskkill', ['/pid', String(child.pid), '/T', '/F'], { stdio: 'ignore' })
  } catch { /* ignore */ }
}

const routes = [
  { name: 'analyze', hash: '#/analyze' },
  { name: 'settings', hash: '#/settings' },
  { name: 'about', hash: '#/about' }
]

const report = { pages: {}, failures: [], consoleErrors: [] }

async function measure (page) {
  return page.evaluate(() => {
    const pick = (selector) => {
      const el = document.querySelector(selector)
      if (!el) return null
      const rect = el.getBoundingClientRect()
      const style = getComputedStyle(el)
      return {
        w: Math.round(rect.width),
        h: Math.round(rect.height),
        clientW: el.clientWidth,
        clientH: el.clientHeight,
        scrollW: el.scrollWidth,
        scrollH: el.scrollHeight,
        overflowY: style.overflowY,
        overflowX: style.overflowX
      }
    }
    return {
      innerW: window.innerWidth,
      innerH: window.innerHeight,
      docScrollW: document.documentElement.scrollWidth,
      docScrollH: document.documentElement.scrollHeight,
      page: pick('.page'),
      pageScroll: pick('.page-scroll'),
      inspector: pick('.inspector'),
      grid: pick('.editor-grid'),
      timeline: pick('.timeline-root'),
      settingsScroll: pick('.settings-scroll'),
      bodyOverflow: getComputedStyle(document.body).overflow
    }
  })
}

async function main () {
  mkdirSync(shotsDir, { recursive: true })
  const server = await ensureServer()
  let browser
  try {
    browser = await chromium.launch()
  } catch {
    // 未下载 Playwright Chromium 时退回系统 Edge
    browser = await chromium.launch({ channel: 'msedge' })
  }
  try {
    const context = await browser.newContext({ viewport, deviceScaleFactor: 1, locale: 'zh-CN' })
    const page = await context.newPage()
    page.on('console', (msg) => {
      if (msg.type() === 'error') report.consoleErrors.push(`[console] ${msg.text()}`)
    })
    page.on('pageerror', (err) => report.consoleErrors.push(`[pageerror] ${err.message}`))

    for (const route of routes) {
      await page.goto(`${baseUrl}/${route.hash}`, { waitUntil: 'networkidle' })
      await page.waitForTimeout(600)
      const metrics = await measure(page)
      report.pages[route.name] = metrics
      await page.screenshot({ path: join(shotsDir, `${route.name}.png`) })

      if (metrics.docScrollW > metrics.innerW + 2) {
        report.failures.push(`${route.name}: document horizontal overflow (${metrics.docScrollW} > ${metrics.innerW})`)
      }
      if (metrics.inspector && metrics.inspector.scrollW > metrics.inspector.clientW + 2) {
        report.failures.push(`${route.name}: inspector horizontal overflow (${metrics.inspector.scrollW} > ${metrics.inspector.clientW})`)
      }
    }

    // 设置页滚动行为测试
    await page.goto(`${baseUrl}/#/settings`, { waitUntil: 'networkidle' })
    await page.waitForTimeout(500)
    const before = await page.evaluate(() => {
      const el = document.querySelector('.page-scroll')
      return { scrollTop: el?.scrollTop ?? -1, scrollH: el?.scrollHeight ?? -1, clientH: el?.clientHeight ?? -1 }
    })
    await page.mouse.move(viewport.width / 2, viewport.height / 2)
    await page.mouse.wheel(0, 700)
    await page.waitForTimeout(400)
    const after = await page.evaluate(() => {
      const el = document.querySelector('.page-scroll')
      return { scrollTop: el?.scrollTop ?? -1 }
    })
    report.scrollTest = { before, after }
    await page.screenshot({ path: join(shotsDir, 'settings-scrolled.png') })
    if (!(before.scrollH > before.clientH)) {
      report.failures.push(`settings: content does not overflow (scrollH ${before.scrollH} <= clientH ${before.clientH})`)
    } else if (!(after.scrollTop > before.scrollTop)) {
      report.failures.push(`settings: wheel did not scroll (scrollTop ${before.scrollTop} -> ${after.scrollTop})`)
    }
  } finally {
    await browser.close()
    killTree(server)
  }

  console.log(JSON.stringify(report, null, 2))
  if (report.failures.length || report.consoleErrors.length) {
    console.error(`\nUI check failed: ${report.failures.length} layout issue(s), ${report.consoleErrors.length} console error(s)`)
    process.exit(1)
  }
  console.log('\nUI check passed')
}

main().catch((err) => {
  console.error(err)
  process.exit(1)
})
