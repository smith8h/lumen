// A tiny D-Bus service so the keyboard shortcut can toggle Lumen in a few milliseconds
// (`gdbus call … Toggle`) instead of starting a second Electron process.
import dbus from 'dbus-next'
import { DBUS_IFACE, DBUS_NAME, DBUS_PATH } from '@shared/constants'

export interface DbusHandlers {
  toggle(): void
  show(query: string): void
  hide(): void
  /** Development only: save a PNG of the panel (used for screenshots and visual checks). */
  capture?(path: string): Promise<void>
  /** Development only: press a key in the panel, e.g. "Down", "Enter", "ctrl+K". */
  key?(combo: string): void
}

export async function startDbus(h: DbusHandlers): Promise<boolean> {
  try {
    const bus = dbus.sessionBus()
    const { Interface } = dbus.interface

    class LumenInterface extends Interface {
      Toggle(): void {
        h.toggle()
      }
      Show(query: string): void {
        h.show(query)
      }
      Hide(): void {
        h.hide()
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
        Capture: { inSignature: 's', outSignature: '' },
        Key: { inSignature: 's', outSignature: '' }
      }
    })

    const reply = await bus.requestName(DBUS_NAME, dbus.NameFlag.DO_NOT_QUEUE)
    if (reply !== dbus.RequestNameReply.PRIMARY_OWNER && reply !== dbus.RequestNameReply.ALREADY_OWNER) {
      console.warn('[dbus] name already taken:', DBUS_NAME)
      return false
    }
    bus.export(DBUS_PATH, new LumenInterface(DBUS_IFACE))
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
