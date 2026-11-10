// "> command" — run shell commands, either inline (output streams into the panel)
// or in the user's terminal emulator.
import { spawn, type ChildProcess } from 'node:child_process'
import { existsSync } from 'node:fs'
import { homedir } from 'node:os'
import type { CommandOutputEvent, ResultItem } from '@shared/types'
import { cleanEnv, pathExecutables, spawnDetached, which } from '../env'
import { fuzzyMatch } from '../fuzzy'
import { getCommandHistory, getSettings } from '../store'

type TerminalSpec = { bin: string; args: (cwd: string, argv: string[]) => string[] }

const TERMINALS: TerminalSpec[] = [
  { bin: 'ptyxis', args: (cwd, argv) => ['--new-window', '-d', cwd, ...(argv.length ? ['--', ...argv] : [])] },
  { bin: 'gnome-terminal', args: (cwd, argv) => [`--working-directory=${cwd}`, ...(argv.length ? ['--', ...argv] : [])] },
  { bin: 'kgx', args: (cwd, argv) => [`--working-directory=${cwd}`, ...(argv.length ? ['--', ...argv] : [])] },
  { bin: 'konsole', args: (cwd, argv) => ['--workdir', cwd, ...(argv.length ? ['-e', ...argv] : [])] },
  { bin: 'xfce4-terminal', args: (cwd, argv) => [`--working-directory=${cwd}`, ...(argv.length ? ['-x', ...argv] : [])] },
  { bin: 'tilix', args: (cwd, argv) => ['-w', cwd, ...(argv.length ? ['-x', ...argv] : [])] },
  { bin: 'ghostty', args: (cwd, argv) => [`--working-directory=${cwd}`, ...(argv.length ? ['-e', ...argv] : [])] },
  { bin: 'wezterm', args: (cwd, argv) => ['start', '--cwd', cwd, ...(argv.length ? ['--', ...argv] : [])] },
  { bin: 'kitty', args: (cwd, argv) => ['--directory', cwd, ...argv] },
  { bin: 'alacritty', args: (cwd, argv) => ['--working-directory', cwd, ...(argv.length ? ['-e', ...argv] : [])] },
  { bin: 'foot', args: (cwd, argv) => ['-D', cwd, ...argv] },
  { bin: 'x-terminal-emulator', args: (_cwd, argv) => (argv.length ? ['-e', ...argv] : []) },
  { bin: 'xterm', args: (_cwd, argv) => (argv.length ? ['-e', ...argv] : []) }
]

export const KNOWN_TERMINALS = TERMINALS.map((t) => t.bin)

function pickTerminal(): TerminalSpec | null {
  const preferred = getSettings().terminal
  if (preferred && preferred !== 'auto') {
    const spec = TERMINALS.find((t) => t.bin === preferred)
    if (spec && which(spec.bin)) return spec
    // A custom terminal binary: assume the common `-e` convention.
    if (which(preferred)) return { bin: preferred, args: (_c, argv) => (argv.length ? ['-e', ...argv] : []) }
  }
  const fromEnv = process.env.TERMINAL
  if (fromEnv) {
    const spec = TERMINALS.find((t) => t.bin === fromEnv)
    if (spec && which(spec.bin)) return spec
  }
  return TERMINALS.find((t) => which(t.bin)) ?? null
}

export const userShell = (): string => {
  const s = process.env.SHELL
  return s && existsSync(s) ? s : '/bin/bash'
}

/** Open the user's terminal at `cwd`, optionally running `command` and keeping the shell open afterwards. */
export async function openInTerminal(command: string | null, cwd = homedir()): Promise<void> {
  const term = pickTerminal()
  if (!term) throw new Error('No terminal emulator found')
  const shell = userShell()
  const argv = command ? [shell, '-ic', `${command}; exec ${shell}`] : []
  await spawnDetached(term.bin, term.args(cwd, argv), { cwd })
}

// Programs that need a real TTY; these always run in a terminal window.
const INTERACTIVE = new Set([
  'vim', 'vi', 'nvim', 'nano', 'emacs', 'micro', 'helix', 'hx', 'htop', 'top', 'btop', 'atop', 'nvtop', 'less', 'more',
  'man', 'ssh', 'mosh', 'tmux', 'screen', 'mc', 'ranger', 'nnn', 'lf', 'yazi', 'sudo', 'su', 'passwd', 'watch', 'bash',
  'zsh', 'fish', 'sh', 'python', 'python3', 'node', 'irb', 'psql', 'mysql', 'sqlite3', 'ftp', 'telnet', 'claude', 'lazygit'
])

export function needsTerminal(command: string): boolean {
  const first = command.trim().split(/\s+/)[0] ?? ''
  if (INTERACTIVE.has(first)) return true
  return /\s-f\b|--follow\b|\|\s*less\b/.test(command) && /journalctl|tail|less/.test(command)
}

function commandItem(command: string, score: number, fromHistory = false): ResultItem {
  const interactive = needsTerminal(command)
  const inTerminal = { type: 'run-command', command, mode: 'terminal' } as const
  const inline = { type: 'run-command', command, mode: 'inline' } as const
  return {
    id: `cmd:${command}`,
    kind: 'command',
    title: command,
    subtitle: interactive ? 'Run in terminal' : fromHistory ? 'Recent command · Enter shows output here' : 'Run and show output here',
    icon: { type: 'glyph', name: 'terminal' },
    score,
    action: interactive ? inTerminal : inline,
    actions: interactive
      ? [{ label: 'Run here and show output', action: inline, glyph: 'terminal' }]
      : [{ label: 'Run in terminal', hint: 'Ctrl+Enter', action: inTerminal, glyph: 'terminal' }]
  }
}

/** Results for an explicit "> command" query. */
export function commandResults(command: string): ResultItem[] {
  const cmd = command.trim()
  const items: ResultItem[] = []
  if (cmd) items.push(commandItem(cmd, 2000))
  for (const h of getCommandHistory()) {
    if (h === cmd) continue
    const m = cmd ? fuzzyMatch(cmd, h) : { score: 500, indexes: [] }
    if (m && m.score > 300) items.push(commandItem(h, 1000 + m.score / 10, true))
    if (items.length >= 7) break
  }
  if (!cmd) {
    items.unshift({
      id: 'cmd:open-terminal',
      kind: 'command',
      title: 'Open Terminal',
      subtitle: 'Type a command after ">" to run it',
      icon: { type: 'glyph', name: 'terminal' },
      score: 1500,
      action: { type: 'open-terminal-at', path: homedir() }
    })
  }
  return items
}

/** A low-priority "run it" suggestion when the first word is a program on PATH. */
export function bareCommandResult(query: string): ResultItem | null {
  const first = query.trim().split(/\s+/)[0]
  if (!first || first.length < 2 || !pathExecutables().has(first)) return null
  return { ...commandItem(query.trim(), 300), subtitle: `Run “${first}” · tip: start with > to run commands` }
}

// ── Inline runs ────────────────────────────────────────────────────────────

const running = new Map<string, ChildProcess>()
const MAX_OUTPUT = 512 * 1024

export function runInline(command: string, emit: (e: CommandOutputEvent) => void): string {
  const runId = Math.random().toString(36).slice(2)
  const child = spawn(userShell(), ['-c', command], {
    cwd: homedir(),
    env: cleanEnv({ TERM: 'dumb', PAGER: 'cat', GIT_PAGER: 'cat', SYSTEMD_PAGER: '' }),
    detached: true,
    stdio: ['ignore', 'pipe', 'pipe']
  })
  running.set(runId, child)
  let sent = 0
  const forward = (stream: 'stdout' | 'stderr') => (buf: Buffer) => {
    if (sent > MAX_OUTPUT) return
    sent += buf.length
    const chunk = sent > MAX_OUTPUT ? buf.toString('utf8') + '\n… output truncated\n' : buf.toString('utf8')
    emit({ runId, chunk, stream })
  }
  child.stdout?.on('data', forward('stdout'))
  child.stderr?.on('data', forward('stderr'))
  child.on('error', (err) => emit({ runId, chunk: String(err.message) + '\n', stream: 'stderr' }))
  child.on('close', (code) => {
    running.delete(runId)
    emit({ runId, done: true, exitCode: code })
  })
  return runId
}

export function killRun(runId: string): void {
  const child = running.get(runId)
  if (!child?.pid) return
  try {
    process.kill(-child.pid, 'SIGTERM') // the whole process group
  } catch {
    child.kill('SIGTERM')
  }
}
