import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

const HOVER_HIDE_MS = 2500

// New users see a faster auto-hide (less intrusive) until they've used the pill 10+ times.
// After 10 confident uses the full 8s is earned — they know what the pill is.
function getAutoHideMs(): number {
  try {
    const count = parseInt(localStorage.getItem('pill_show_count') || '0', 10)
    return count < 10 ? 3000 : 8000
  } catch { return 8000 }
}

function incrementPillCount() {
  try {
    const count = parseInt(localStorage.getItem('pill_show_count') || '0', 10)
    localStorage.setItem('pill_show_count', String(count + 1))
  } catch { /* storage unavailable */ }
}

let hideTimer: ReturnType<typeof setTimeout>
let isWorking = false

const LABELS: Record<string, string> = { Reply: 'Reply', Do: 'Do', Correct: 'Fix', Prompt: 'Prompt' }

function resetAutoHide() {
  clearTimeout(hideTimer)
  hideTimer = setTimeout(() => invoke('hide_pill'), getAutoHideMs())
}

function setWorking(mode: string) {
  isWorking = true
  document.querySelectorAll<HTMLElement>('.pill-btn').forEach(b => {
    b.style.opacity = '0.35'
    b.style.pointerEvents = 'none'
    if (b.dataset.mode === mode) {
      b.textContent = '…'
      b.style.opacity = '0.7'
      b.style.color = '#a5b4fc'
    }
  })
}

function resetWorking() {
  isWorking = false
  document.querySelectorAll<HTMLElement>('.pill-btn[data-mode]').forEach(b => {
    b.textContent = LABELS[b.dataset.mode || ''] ?? b.dataset.mode ?? ''
    b.style.opacity = ''
    b.style.pointerEvents = ''
    b.style.color = ''
  })
}

// ── Primary reset: fires when the OS window becomes visible again ──────────
// This is the most reliable hook — triggers on w.show() regardless of events.
document.addEventListener('visibilitychange', () => {
  if (!document.hidden) {
    resetWorking()
    resetAutoHide()
  }
})

// ── Secondary: Rust-emitted pill_show (belt + suspenders) ─────────────────
listen('pill_show', () => {
  const el = document.getElementById('pill')
  if (el) el.style.opacity = '1'
  incrementPillCount()
  resetWorking()
  resetAutoHide()
})

listen('pill_hide', () => {
  const el = document.getElementById('pill')
  if (el) el.style.opacity = '0'
  clearTimeout(hideTimer)
  resetWorking()
})

// ── Mode buttons ───────────────────────────────────────────────────────────
document.querySelectorAll<HTMLElement>('.pill-btn[data-mode]').forEach(btn => {
  btn.addEventListener('click', async () => {
    if (isWorking) return
    clearTimeout(hideTimer)
    const mode = btn.dataset.mode || 'Reply'
    setWorking(mode)
    // pill_clicked is a non-async Rust fn — invoke resolves as soon as Rust
    // hides the pill and spawns the background task. Reset state here so the
    // next show always starts clean, regardless of event delivery.
    try { await invoke('pill_clicked', { mode }) } finally { resetWorking() }
  })
})

// ── Dismiss ────────────────────────────────────────────────────────────────
document.getElementById('pill-dismiss')?.addEventListener('click', () => {
  clearTimeout(hideTimer)
  invoke('hide_pill')
})

// ── Escape closes the pill ─────────────────────────────────────────────────
document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') {
    clearTimeout(hideTimer)
    invoke('hide_pill')
  }
})

// ── Hover pause ───────────────────────────────────────────────────────────
document.getElementById('pill')?.addEventListener('mouseenter', () => {
  clearTimeout(hideTimer)
})

document.getElementById('pill')?.addEventListener('mouseleave', () => {
  if (!isWorking) {
    hideTimer = setTimeout(() => invoke('hide_pill'), HOVER_HIDE_MS)
  }
})

resetAutoHide()
