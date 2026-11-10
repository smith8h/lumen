import { contextBridge, ipcRenderer, type IpcRendererEvent } from 'electron'
import type { LumenApi } from '@shared/types'

function on<T>(channel: string, cb: (payload: T) => void): () => void {
  const listener = (_e: IpcRendererEvent, payload: T) => cb(payload)
  ipcRenderer.on(channel, listener)
  return () => ipcRenderer.removeListener(channel, listener)
}

const api: LumenApi = {
  search: (query, seq) => ipcRenderer.invoke('search', query, seq),
  suggestions: () => ipcRenderer.invoke('suggestions'),
  execute: (item) => ipcRenderer.invoke('execute', { action: item.action, historyKey: item.historyKey }),
  preview: (item) => ipcRenderer.invoke('preview', { id: item.id, kind: item.kind, action: item.action, title: item.title, subtitle: item.subtitle }),
  killCommand: (runId) => ipcRenderer.invoke('kill', runId),
  hide: () => ipcRenderer.send('hide'),
  getSettings: () => ipcRenderer.invoke('settings:get'),
  setSettings: (patch) => ipcRenderer.invoke('settings:set', patch),
  shortcutStatus: () => ipcRenderer.invoke('shortcut:status'),
  setupShortcut: () => ipcRenderer.invoke('shortcut:setup'),
  removeShortcut: () => ipcRenderer.invoke('shortcut:remove'),
  appInfo: () => ipcRenderer.invoke('app:info'),
  onShow: (cb) => on('lumen:show', cb),
  onHide: (cb) => on('lumen:hide', () => cb()),
  onCommandOutput: (cb) => on('lumen:command-output', cb),
  onSettings: (cb) => on('lumen:settings', cb)
}

contextBridge.exposeInMainWorld('lumen', api)
