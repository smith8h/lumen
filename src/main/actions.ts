// Executes result actions. Everything coming from the renderer is re-validated here.
import { app, shell, type WebContents } from 'electron'
import { existsSync } from 'node:fs'
import type { Action, ExecuteResult, Preview, ResultItem } from '@shared/types'
import { clearClips, clipPreview, deleteClip, getClip, pinClip, touchClip, writeClipboard } from './clipboard'
import { spawnDetached, which } from './env'
import { filePreview, reindex } from './files'
import { appPreview, launchApp } from './providers/apps'
import { pasteText } from './paste'
import { killRun, openInTerminal, runInline } from './providers/terminal'
import { runSystem } from './providers/system'
import { recordCommand, recordUse } from './store'

const SAFE_URL = /^(https?|ftp|mailto):/i

async function openPath(path: string): Promise<void> {
  if (!existsSync(path)) throw new Error('File no longer exists')
  if (which('gio')) await spawnDetached('gio', ['open', path])
  else await spawnDetached('xdg-open', [path])
}

export async function execute(item: Pick<ResultItem, 'action' | 'historyKey'>, sender: WebContents): Promise<ExecuteResult> {
  const a = item.action as Action
  const remember = () => {
    if (typeof item.historyKey === 'string' && item.historyKey.length < 4096) recordUse(item.historyKey)
  }
  try {
    switch (a.type) {
      case 'launch-app':
        await launchApp(String(a.desktopId), a.actionId ? String(a.actionId) : undefined)
        remember()
        return { ok: true }
      case 'open-path':
        await openPath(String(a.path))
        remember()
        return { ok: true }
      case 'reveal-path':
        shell.showItemInFolder(String(a.path))
        remember()
        return { ok: true }
      case 'open-terminal-at':
        await openInTerminal(null, existsSync(String(a.path)) ? String(a.path) : undefined)
        return { ok: true }
      case 'copy':
        await writeClipboard(String(a.text))
        remember()
        return { ok: true, toast: 'Copied to clipboard' }
      case 'clip-paste': {
        const clip = getClip(String(a.id))
        if (!clip) return { ok: false, error: 'That clip is gone' }
        touchClip(clip.id)
        return pasteText(clip.text, { terminal: a.terminal === true ? true : undefined })
      }
      case 'clip-copy': {
        const clip = getClip(String(a.id))
        if (!clip) return { ok: false, error: 'That clip is gone' }
        await writeClipboard(clip.text)
        touchClip(clip.id)
        return { ok: true, toast: 'Copied to clipboard' }
      }
      case 'clip-pin':
        if (!pinClip(String(a.id), a.pinned === true)) return { ok: false, error: 'That clip is gone' }
        return { ok: true, keepOpen: true, refresh: true, toast: a.pinned === true ? 'Pinned' : 'Unpinned' }
      case 'clip-delete':
        if (!deleteClip(String(a.id))) return { ok: false, error: 'That clip is gone' }
        return { ok: true, keepOpen: true, refresh: true, toast: 'Deleted' }
      case 'clip-clear': {
        const n = clearClips()
        return { ok: true, toast: n ? `Cleared ${n} clip${n === 1 ? '' : 's'}` : 'Clipboard history is already empty' }
      }
      case 'open-url': {
        const url = String(a.url)
        if (!SAFE_URL.test(url)) throw new Error('Unsupported link')
        await spawnDetached('xdg-open', [url])
        remember()
        return { ok: true }
      }
      case 'run-command': {
        const command = String(a.command).trim()
        if (!command) return { ok: false, error: 'Empty command' }
        recordCommand(command)
        if (a.mode === 'terminal') {
          await openInTerminal(command)
          return { ok: true }
        }
        const runId = runInline(command, (e) => {
          if (!sender.isDestroyed()) sender.send('lumen:command-output', e)
        })
        return { ok: true, keepOpen: true, runId }
      }
      case 'system': {
        remember()
        const toast = await runSystem(a.op)
        return { ok: true, toast }
      }
      case 'lumen':
        switch (a.op) {
          case 'settings':
            return { ok: true, keepOpen: true, view: 'settings' }
          case 'shortcut':
            return { ok: true, keepOpen: true, view: 'shortcut' }
          case 'reindex':
            reindex()
            return { ok: true, keepOpen: false, toast: 'Rebuilding file index…' }
          case 'quit':
            setImmediate(() => app.quit())
            return { ok: true }
        }
    }
    return { ok: false, error: 'Unknown action' }
  } catch (err) {
    return { ok: false, error: err instanceof Error ? err.message : String(err) }
  }
}

export async function preview(item: Pick<ResultItem, 'kind' | 'action' | 'title' | 'subtitle'> & { id?: string }): Promise<Preview> {
  const a = item.action
  if (a.type === 'launch-app') return appPreview(a.desktopId)
  if (a.type === 'open-path') return filePreview(a.path)
  if (a.type === 'clip-paste') return clipPreview(a.id)
  if (item.kind === 'calc') {
    const expression = item.id?.startsWith('calc:') ? item.id.slice(5) : (item.subtitle ?? '').replace(/\s*=$/, '')
    return { type: 'calc', expression, result: item.title, detail: item.subtitle }
  }
  return { type: 'none' }
}

export const killCommand = (runId: string): void => killRun(String(runId))
