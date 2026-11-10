#!/usr/bin/env node
// Packs gnome-extension/ into an extensions.gnome.org-ready zip in release/.
//
//   npm run extension:pack       → release/lumen@lumenlauncher.github.io.shell-extension.zip
//   npm run extension:install    → also installs it for the current user (log out and back in to load it)
//
// Uses `gnome-extensions pack` when it's available (it validates metadata.json), otherwise
// plain `zip`, so it also works on CI machines without GNOME Shell.
const { execFileSync, spawnSync } = require('node:child_process')
const { existsSync, mkdirSync, readFileSync, readdirSync, rmSync } = require('node:fs')
const { join } = require('node:path')

const root = join(__dirname, '..')
const src = join(root, 'gnome-extension')
const outDir = join(root, 'release')
const { uuid } = JSON.parse(readFileSync(join(src, 'metadata.json'), 'utf8'))
const zip = join(outDir, `${uuid}.shell-extension.zip`)

const has = (cmd) => spawnSync('sh', ['-c', `command -v ${cmd}`], { stdio: 'ignore' }).status === 0

// Everything the shell loads; docs and dev files stay out of the bundle.
const files = readdirSync(src).filter((f) => f === 'metadata.json' || f.endsWith('.js') || f === 'stylesheet.css')
if (!files.includes('extension.js')) throw new Error('gnome-extension/extension.js is missing')

mkdirSync(outDir, { recursive: true })
rmSync(zip, { force: true })

if (has('gnome-extensions')) {
  const defaults = new Set(['metadata.json', 'extension.js', 'prefs.js', 'stylesheet.css'])
  const extra = files.filter((f) => !defaults.has(f)).map((f) => `--extra-source=${f}`)
  execFileSync('gnome-extensions', ['pack', '--force', `--out-dir=${outDir}`, ...extra, '.'], { cwd: src, stdio: 'inherit' })
} else if (has('zip')) {
  execFileSync('zip', ['-q', '-X', zip, ...files], { cwd: src, stdio: 'inherit' })
} else {
  throw new Error('Needs either `gnome-extensions` (GNOME Shell) or `zip` to pack the extension')
}
if (!existsSync(zip)) throw new Error(`Packing failed: ${zip} was not created`)
console.log(`Packed ${files.join(', ')}\n→ ${zip}`)

if (process.argv.includes('--install')) {
  execFileSync('gnome-extensions', ['install', '--force', zip], { stdio: 'inherit' })
  console.log(
    `\nInstalled ${uuid}.\nOn Wayland GNOME only loads new extensions at login: log out and back in, then run\n` +
      `  gnome-extensions enable ${uuid}\n(or turn it on in the Extensions app).`
  )
}
