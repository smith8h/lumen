// electron-builder hook: rename the Electron binary to `lumen-bin` and put the launcher
// script in its place, so .deb, AppImage and the menu entry all go through it.
const { chmodSync, copyFileSync, existsSync, renameSync } = require('node:fs')
const { join } = require('node:path')

exports.default = async function afterPack(context) {
  if (context.electronPlatformName !== 'linux') return
  const name = context.packager.executableName
  const dir = context.appOutDir
  const bin = join(dir, name)
  if (!existsSync(bin)) throw new Error(`afterPack: ${bin} not found`)
  renameSync(bin, join(dir, `${name}-bin`))
  copyFileSync(join(__dirname, 'linux/lumen-launcher.sh'), bin)
  chmodSync(bin, 0o755)
}
