// A tiny D-Bus service so the keyboard shortcut can toggle Lumen in a few milliseconds
// (`gdbus call … Toggle`) instead of starting a second Electron process.
//
// It also talks to the optional GNOME Shell extension (gnome-extension/):
//   • signal PanelGeometry(x, y, w, h, radius): where the panel is inside Lumen's window, in
//     CSS pixels, so the extension can blur exactly that area. All zeros = panel closed.
//   • property ShellBlur (read/write bool): the extension sets it while it draws real blur,
//     and Lumen makes its own tint more see-through.
//   • method PreviousWindow(wm_class, is_x11): when the panel takes focus, the extension says
//     which window had it before, so pasting a clip into a terminal uses Ctrl+Shift+V and
//     an XWayland window can be reached with xdotool.
//   • method ClipboardSecret(): a copy was tagged as a secret by a password manager
//     (x-kde-passwordManagerHint), so clipboard history skips it. Never carries the text.
import dbus from 'dbus-next'
import type { PanelRect } from '@shared/types'
import { DBUS_IFACE, DBUS_NAME, DBUS_PATH } from '@shared/constants'

export interface DbusHandlers {
  toggle(): void
  show(query: string): void
  hide(): void
  /** Development only: save a PNG of the panel (used for screenshots and visual checks). */
  capture?(path: string): Promise<void>
  /** Development only: press a key in the panel, e.g. "Down", "Enter", "ctrl+K". */
  key?(combo: string): void
  /** The GNOME extension started or stopped drawing real blur behind the panel. */
  shellBlurChanged(enabled: boolean): void
  /** The GNOME extension reports the window that had focus before the panel. */
  previousWindow(wmClass: string, x11: boolean): void
  /** The GNOME extension saw a copy marked as a secret. */
  clipboardSecret(): void
}

type Service = { PanelGeometry(rect: PanelRect | null): void }
let service: Service | null = null
let lastRect: PanelRect | null = null
let lastSent = ''

/** Broadcast the panel's rectangle (or null when it's closed) for the GNOME extension. */
export function emitPanelGeometry(rect: PanelRect | null): void {
  lastRect = rect
  const key = rect ? `${rect.x},${rect.y},${rect.width},${rect.height},${rect.radius}` : ''
  if (key === lastSent) return
  lastSent = key
  service?.PanelGeometry(rect)
}

export async function startDbus(h: DbusHandlers): Promise<boolean> {
  try {
    const bus = dbus.sessionBus()
    const { Interface } = dbus.interface

    class LumenInterface extends Interface {
      private shellBlur = false

      get ShellBlur(): boolean {
        return this.shellBlur
      }
      set ShellBlur(value: boolean) {
        value = !!value
        if (value !== this.shellBlur) {
          this.shellBlur = value
          Interface.emitPropertiesChanged(this, { ShellBlur: value }, [])
          h.shellBlurChanged(value)
        }
        // The extension (re)connected: give it the current panel straight away.
        if (value) {
          lastSent = '\u0000'
          emitPanelGeometry(lastRect)
        }
      }

      PanelGeometry(rect: PanelRect | null): number[] {
        return rect ? [rect.x, rect.y, rect.width, rect.height, rect.radius] : [0, 0, 0, 0, 0]
      }

      Toggle(): void {
        h.toggle()
      }
      Show(query: string): void {
        h.show(query)
      }
      Hide(): void {
        h.hide()
      }
      ClipboardSecret(): void {
        h.clipboardSecret()
      }
      PreviousWindow(wmClass: string, x11: boolean): void {
        if (typeof wmClass === 'string' && wmClass.length <= 256) h.previousWindow(wmClass, !!x11)
      }
      Key(combo: string): void {
        if (!h.key) throw new Error('Key is only available in development builds')
        h.key(combo)
      }
      async Capture(path: string): Promise<void> {
        if (!h.capture) throw new Error('Capture is only available in development builds')
        await h.capture(path)
      }
    }
    LumenInterface.configureMembers({
      methods: {
        Toggle: { inSignature: '', outSignature: '' },
        Show: { inSignature: 's', outSignature: '' },
        Hide: { inSignature: '', outSignature: '' },
        PreviousWindow: { inSignature: 'sb', outSignature: '' },
        ClipboardSecret: { inSignature: '', outSignature: '' },
        Capture: { inSignature: 's', outSignature: '' },
        Key: { inSignature: 's', outSignature: '' }
      },
      properties: {
        ShellBlur: { signature: 'b', access: dbus.interface.ACCESS_READWRITE }
      },
      signals: {
        PanelGeometry: { signature: 'iiiii' }
      }
    })

    // Export before taking the name: clients (like the GNOME extension) call in as soon as it appears.
    const iface = new LumenInterface(DBUS_IFACE)
    bus.export(DBUS_PATH, iface)
    const reply = await bus.requestName(DBUS_NAME, dbus.NameFlag.DO_NOT_QUEUE)
    if (reply !== dbus.RequestNameReply.PRIMARY_OWNER && reply !== dbus.RequestNameReply.ALREADY_OWNER) {
      console.warn('[dbus] name already taken:', DBUS_NAME)
      bus.unexport(DBUS_PATH, iface)
      return false
    }
    service = iface
    return true
  } catch (err) {
    console.warn('[dbus] unavailable, falling back to single-instance toggling:', err)
    return false
  }
}

/** Shell command that toggles Lumen, starting it if it isn't running yet. */
export function toggleCommand(launcher: string): string {
  const q = (s: string) => `'${s.replace(/'/g, `'\\''`)}'`
  return `gdbus call --session --dest ${DBUS_NAME} --object-path ${DBUS_PATH} --method ${DBUS_IFACE}.Toggle >/dev/null 2>&1 || exec ${launcher
    .split('\u0000')
    .map(q)
    .join(' ')} --toggle`
}
