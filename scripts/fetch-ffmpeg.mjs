// 准备内置 FFmpeg：优先复用本地副本，其次下载 BtbN GPL 构建。
import { existsSync, mkdirSync, copyFileSync, createWriteStream, rmSync } from 'node:fs'
import { spawnSync } from 'node:child_process'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { get } from 'node:https'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const binDir = join(root, 'src-tauri', 'ffmpeg', 'bin')
const targets = ['ffmpeg.exe', 'ffprobe.exe']
const url = 'https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip'

function complete (dir) {
  return targets.every((name) => existsSync(join(dir, name)))
}

function copyFrom (dir, label) {
  if (!complete(dir)) return false
  mkdirSync(binDir, { recursive: true })
  for (const name of targets) {
    copyFileSync(join(dir, name), join(binDir, name))
  }
  console.log(`[fetch-ffmpeg] 已从 ${label} 复制 ffmpeg/ffprobe`)
  return true
}

function download (file) {
  return new Promise((resolvePromise, reject) => {
    const request = get(url, (response) => {
      if (response.statusCode >= 300 && response.statusCode < 400 && response.headers.location) {
        request.destroy()
        download(response.headers.location).then(resolvePromise, reject)
        return
      }
      if (response.statusCode !== 200) {
        reject(new Error(`HTTP ${response.statusCode}`))
        return
      }
      const out = createWriteStream(file)
      response.pipe(out)
      out.on('finish', () => out.close(resolvePromise))
      out.on('error', reject)
    })
    request.on('error', reject)
  })
}

async function main () {
  if (complete(binDir)) {
    console.log('[fetch-ffmpeg] 已存在，跳过')
    return
  }
  // 1) 复用同机 AudioSeeker 的内置副本
  const sibling = resolve(root, '..', 'CyreneAudioSeeker', 'src-tauri', 'ffmpeg', 'bin')
  if (copyFrom(sibling, 'CyreneAudioSeeker')) return

  // 2) 下载 BtbN GPL 构建
  const tmp = join(root, '.local', 'ffmpeg-download')
  rmSync(tmp, { recursive: true, force: true })
  mkdirSync(tmp, { recursive: true })
  const zip = join(tmp, 'ffmpeg.zip')
  console.log('[fetch-ffmpeg] 下载', url)
  await download(zip)
  console.log('[fetch-ffmpeg] 解压…')
  const result = spawnSync('powershell', [
    '-NoProfile',
    '-Command',
    `Expand-Archive -LiteralPath '${zip}' -DestinationPath '${join(tmp, 'extract')}' -Force`
  ], { stdio: 'inherit' })
  if (result.status !== 0) throw new Error('解压失败')

  mkdirSync(binDir, { recursive: true })
  const found = spawnSync('powershell', [
    '-NoProfile',
    '-Command',
    `$f = Get-ChildItem -Recurse -Filter ffmpeg.exe '${join(tmp, 'extract')}' | Select-Object -First 1; Copy-Item $f.FullName '${join(binDir, 'ffmpeg.exe')}' -Force; ` +
    `$p = Get-ChildItem -Recurse -Filter ffprobe.exe '${join(tmp, 'extract')}' | Select-Object -First 1; Copy-Item $p.FullName '${join(binDir, 'ffprobe.exe')}' -Force`
  ], { stdio: 'inherit' })
  if (found.status !== 0) throw new Error('复制 ffmpeg 失败')
  if (!complete(binDir)) throw new Error('ffmpeg/ffprobe 不完整')
  console.log('[fetch-ffmpeg] 完成')
}

main().catch((err) => {
  console.error('[fetch-ffmpeg]', err.message)
  process.exit(1)
})
