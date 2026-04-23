/**
 * Tauri v2 mock for Playwright browser context.
 *
 * Tauri v2 listen() flow:
 *   1. transformCallback(handler, once) → stores fn, returns numeric id
 *   2. invoke('plugin:event|listen', { event, handler: id }) → registers id under event name
 *   3. invoke('plugin:event|unlisten', ...) → cleanup
 *
 * Our __emit(event, payload) looks up the registered IDs and calls their stored fn.
 *
 * Usage in tests:
 *   await page.addInitScript({ content: buildTauriInitScript() })
 *   await page.evaluate(([e,p]) => window.__TAURI_MOCK__.__emit(e,p), [event, payload])
 */

export function buildTauriInitScript(overrides: Record<string, unknown> = {}): string {
  const defaults = {
    has_api_key: false,
    get_ai_mode: 'Worker',
    get_worker_usage: { used: 3, cap: 20 },
    get_history: [],
    get_communication_score: {
      avg_tone: 1.2,
      avg_formality: 55,
      total_sessions: 14,
      frequent_entities: ['Priya', 'Team'],
      friction_hotspots: [],
    },
    get_hardware_stats: { cpu_count: 8, ram_gb: 16 },
    // '1' = first_run_done exists → skip welcome screen
    get_config_value: '1',
    set_config_value: null,
    set_ai_mode: null,
    store_api_key: null,
    inject_result: null,
    hide_window: null,
    generate_ai_response: null,
    record_reply_feedback: null,
    record_intent_correction: null,
    trigger_onboarding_demo: null,
    get_captured_text: '',
  }

  const merged = { ...defaults, ...overrides }

  return `
(function () {
  const DEFAULT_HANDLERS = ${JSON.stringify(merged)};

  // ── Callback registry (for transformCallback) ──────────────────
  let _nextId = 1;
  const _callbacks = {};     // id → { fn, once }
  const _eventMap = {};      // event name → [id, ...]
  const _userHandlers = {};  // invoke cmd → fn

  // Pre-populate invoke handlers from defaults
  for (const [cmd, val] of Object.entries(DEFAULT_HANDLERS)) {
    _userHandlers[cmd] = () => val;
  }

  // ── Tauri v2 event plugin internals (used by _unlisten in event.js) ────────
  // event.js calls window.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener()
  // before invoking plugin:event|unlisten. We provide a no-op stub.
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: function() {},
  };

  // ── Tauri v2 required interface ──────────────────────────────────
  window.__TAURI_INTERNALS__ = {

    metadata: {
      currentWindow: { label: 'main' },
      windows:  [{ label: 'main' }],
      webviews: [{ label: 'main' }],
    },

    // Called by @tauri-apps/api/core transformCallback()
    transformCallback: function(fn, once) {
      const id = _nextId++;
      _callbacks[id] = { fn, once: !!once };
      return id;
    },

    // invoke() handles both plugin:event|listen and user commands
    invoke: async function(cmd, args, _opts) {
      // ── Tauri event plumbing ─────────────────────────────────────
      if (cmd === 'plugin:event|listen') {
        const event = args.event;
        const handlerId = args.handler;
        // Replace (not push) — React StrictMode double-fires effects; the async
        // setup() populates unlisteners AFTER cleanup runs, so both setups complete
        // and register separate handler IDs. Keeping only the latest prevents
        // listener stacking and token doubling.
        _eventMap[event] = [handlerId];
        return handlerId;
      }
      if (cmd === 'plugin:event|unlisten') {
        const { event, eventId } = args || {};
        if (event && _eventMap[event]) {
          // Remove the handler with matching eventId (best-effort)
          _eventMap[event] = _eventMap[event].filter(id => id !== eventId);
        }
        return null;
      }

      // ── Updater plugin (used by update notification) ─────────────
      if (cmd && cmd.startsWith('plugin:updater|')) return null;

      // ── User-defined handlers ────────────────────────────────────
      const h = _userHandlers[cmd];
      if (h) {
        const result = h(args || {});
        // Allow handlers to throw to simulate missing config keys
        if (result === '__THROW__') throw new Error('not found');
        return result;
      }
      console.warn('[TauriMock] unhandled invoke:', cmd, args);
      return null;
    },

    listen: undefined, // Tauri v2 uses invoke('plugin:event|listen') — not this
  };

  // ── Test helpers ─────────────────────────────────────────────────
  const mock = {

    /** Override an invoke handler dynamically from tests. */
    setHandler: function(cmd, fn) {
      _userHandlers[cmd] = typeof fn === 'function' ? fn : () => fn;
    },

    /** Emit a Tauri event — triggers all registered listen() callbacks. */
    __emit: function(event, payload) {
      const ids = _eventMap[event] || [];
      ids.forEach(function(id) {
        const cb = _callbacks[id];
        if (cb) {
          cb.fn({ event, id: Date.now(), payload });
          if (cb.once) delete _callbacks[id];
        }
      });
    },

    /** Simulate user hotkey text capture (most tests start here). */
    captureText: function(text, suggestedMode, appContext) {
      suggestedMode = suggestedMode || 'Correct';
      appContext = appContext || 'other';
      mock.__emit('text_captured', {
        text: text,
        context: {
          original: text, normalized: text,
          word_count: text.split(' ').length, char_count: text.length,
          language: { primary_script: 'Latin', primary_pct: 100, is_mixed: false, is_rtl: false, candidate_languages: 'en' },
          intent_result: {
            primary: { intent: suggestedMode, confidence: 0.85, label: suggestedMode, mode: suggestedMode, reason: '' },
            alternatives: [],
            overall_confidence: 0.85,
          },
          formality: 50, keywords: [], tone: 1,
          friction_phrases: [], suggested_mode: suggestedMode,
        },
        forced_mode: null,
        app_context: appContext,
      });
    },

    /** Simulate Hinglish / mixed-language text capture. */
    captureHinglish: function(text) {
      mock.__emit('text_captured', {
        text: text,
        context: {
          original: text, normalized: text,
          word_count: text.split(' ').length, char_count: text.length,
          language: { primary_script: 'Latin', primary_pct: 60, is_mixed: true, is_rtl: false, candidate_languages: 'hi,en' },
          intent_result: {
            primary: { intent: 'Translate', confidence: 0.75, label: 'Translate', mode: 'Translate', reason: 'mixed language detected' },
            alternatives: [],
            overall_confidence: 0.75,
          },
          formality: 30, keywords: [], tone: 0,
          friction_phrases: [], suggested_mode: 'Translate',
        },
        forced_mode: null,
        app_context: 'chat',
      });
    },

    /** Stream AI tokens then emit stream_end. delayMs controls token pace. */
    streamResponse: function(text, delayMs) {
      delayMs = delayMs || 40;
      const tokens = text.split(' ');
      let i = 0;
      function next() {
        if (i < tokens.length) {
          mock.__emit('ai_token', (i === 0 ? '' : ' ') + tokens[i++]);
          setTimeout(next, delayMs);
        } else {
          mock.__emit('ai_stream_end', null);
        }
      }
      setTimeout(next, 60);
    },

    /** Simulate a backend error during generation. */
    streamError: function(msg) {
      setTimeout(function() { mock.__emit('ai_error', msg); }, 80);
    },
  };

  window.__TAURI_MOCK__ = mock;
  // Keep backward-compat alias
  window.__TAURI_INTERNALS__.__emit = mock.__emit.bind(mock);
  window.__TAURI_INTERNALS__.setHandler = mock.setHandler.bind(mock);
})();
  `.trim()
}
