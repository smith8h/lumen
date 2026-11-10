// The panel often appears under a resting mouse cursor. Browsers then fire mousemove
// without the mouse moving, which would steal the keyboard selection. Only real
// movement counts as hover.
let last: { x: number; y: number } | null = null

export function pointerMoved(e: { screenX: number; screenY: number }): boolean {
  const moved = last !== null && (last.x !== e.screenX || last.y !== e.screenY)
  last = { x: e.screenX, y: e.screenY }
  return moved
}

export const resetPointer = (): void => {
  last = null
}
