// Renders resources/icon.svg to PNGs with Electron (no ImageMagick needed).
// Usage: npx electron scripts/make-icons.cjs
const { app, BrowserWindow } = require('electron')
const { mkdirSync, readFileSync, writeFileSync } = require('node:fs')
const { join } = require('node:path')

const root = join(__dirname, '..')
const svg = readFileSync(join(root, 'resources/icon.svg'), 'utf8')
const sizes = [16, 24, 32, 48, 64, 128, 256, 512]

app.disableHardwareAcceleration()
app.whenReady().then(async () => {
  mkdirSync(join(root, 'build/icons'), { recursive: true })
  // Render once at 1024 for crisp downscaling, then resize to each size.
  const win = new BrowserWindow({ width: 1024, height: 1024, show: false, transparent: true, frame: false, webPreferences: { offscreen: true } })
  const html = `<html><body style="margin:0;background:transparent">${svg.replace('<svg ', '<svg style="display:block;width:1024px;height:1024px" ')}</body></html>`
  await win.loadURL('data:text/html;charset=utf-8,' + encodeURIComponent(html))
  await new Promise((r) => setTimeout(r, 300))
  const big = await win.webContents.capturePage()
  for (const size of sizes) {
    const png = big.resize({ width: size, height: size, quality: 'best' }).toPNG()
    writeFileSync(join(root, `build/icons/${size}x${size}.png`), png)
    if (size === 512) writeFileSync(join(root, 'resources/icon.png'), png)
  }
  win.destroy()
  console.log('icons written')
  app.quit()
})
