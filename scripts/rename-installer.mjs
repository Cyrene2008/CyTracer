// 将 NSIS 安装包重命名为 CyTracer_<version>_x64-setup.exe
import { existsSync, readdirSync, readFileSync, renameSync, rmSync, statSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const conf = JSON.parse(readFileSync(join(root, 'src-tauri', 'tauri.conf.json'), 'utf8'))
const version = conf.version
const targetName = `CyTracer_${version}_x64-setup.exe`

const candidates = [
  process.env.CARGO_TARGET_DIR
    ? join(process.env.CARGO_TARGET_DIR, 'release', 'bundle', 'nsis')
    : null,
  join(root, 'target', 'release', 'bundle', 'nsis'),
  join(root, 'src-tauri', 'target', 'release', 'bundle', 'nsis')
].filter(Boolean)

let dir = null
for (const candidate of candidates) {
  if (existsSync(candidate)) {
    dir = candidate
    break
  }
}

if (!dir) {
  console.error('[rename-installer] 未找到 NSIS 产物目录')
  process.exit(1)
}

const files = readdirSync(dir).filter((name) => name.endsWith('.exe'))
if (!files.length) {
  console.error(`[rename-installer] ${dir} 下没有安装包`)
  process.exit(1)
}

const match = files.find((name) => name.includes(version)) || files[0]
for (const name of files) {
  if (name !== match && name !== targetName) {
    const stale = join(dir, name)
    if (statSync(stale).isFile()) {
      try { rmSync(stale) } catch { /* 忽略 */ }
    }
  }
}
const source = join(dir, match)
const dest = join(dir, targetName)
if (source !== dest) {
  if (existsSync(dest)) rmSync(dest)
  renameSync(source, dest)
}
console.log(`[rename-installer] ${dest}`)
