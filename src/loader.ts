// Loader toast entry point — used by loader.html (silent hotkey feedback window)
import { listen } from '@tauri-apps/api/event'

const toast = document.getElementById('toast')!
const labelEl = document.getElementById('label')!

listen<{ state: string; label: string }>('loader_state', (event) => {
    const { state, label } = event.payload

    toast.className = 'toast'

    if (state === 'done') {
        toast.classList.add('done')
        labelEl.textContent = '✓ ' + (label || 'Done')
    } else if (state === 'error') {
        toast.classList.add('error')
        labelEl.textContent = '✗ ' + (label || 'Failed')
    } else {
        labelEl.textContent = label || 'Working…'
    }
})