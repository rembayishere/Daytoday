export function matchShortcut(e: KeyboardEvent, shortcut: string): boolean {
  const parts = shortcut.split('+')
  const key = parts.pop()!.toLowerCase()
  const hasCtrl = parts.some((p) => p.toLowerCase() === 'ctrl')
  const hasAlt = parts.some((p) => p.toLowerCase() === 'alt')
  const hasShift = parts.some((p) => p.toLowerCase() === 'shift')
  const hasMeta = parts.some((p) => p.toLowerCase() === 'win' || p.toLowerCase() === 'meta')
  const specialKeys: Record<string, string> = {
    space: ' ', enter: 'Enter', tab: 'Tab',
    arrowup: 'ArrowUp', arrowdown: 'ArrowDown', arrowleft: 'ArrowLeft', arrowright: 'ArrowRight',
  }
  const actualKey = specialKeys[key] || key
  return e.key === actualKey &&
    !!e.ctrlKey === hasCtrl &&
    !!e.altKey === hasAlt &&
    !!e.shiftKey === hasShift &&
    !!e.metaKey === hasMeta
}
