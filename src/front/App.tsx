import { useState, useEffect, useRef } from 'react'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { motion, AnimatePresence } from 'framer-motion'
import { Settings, Sparkles, Send, CheckCircle2, Copy } from 'lucide-react'
// Single CSS import — App.css is intentionally empty, all styles in index.css
import './index.css'

// ── Types ──────────────────────────────────────────────────────────────────

type Mode =
  | 'Prompt' | 'Summarize' | 'Email' | 'Correct'
  | 'Translate' | 'Casual' | 'Professional'
  | 'Strategist' | 'Custom';

interface IntentCandidate {
  intent: string;
  confidence: number;
  label: string;
  mode: Mode;
  reason: string;
}

interface IntentResult {
  primary: IntentCandidate;
  alternatives: IntentCandidate[];
  overall_confidence: number;
}

interface LanguageContext {
  primary_script: string;
  primary_pct: number;
  is_mixed: boolean;
  is_rtl: boolean;
  candidate_languages: string;
}

interface TextContext {
  original: string;
  normalized: string;
  word_count: number;
  char_count: number;
  language: LanguageContext;
  intent_result: IntentResult;
  formality: number;
  keywords: string[];
  tone: number;
  friction_phrases: string[];
  suggested_mode: Mode;
}

// ── TonePill ───────────────────────────────────────────────────────────────

const TonePill = ({ score, friction }: { score: number; friction: string[] }) => {
  const tone = (() => {
    if (friction.length > 0) return { color: '#f59e0b', label: 'Friction detected', icon: '⚠️' };
    if (score >= 3)           return { color: '#10b981', label: 'Positive',          icon: '✦'  };
    if (score <= -2)          return { color: '#ef4444', label: 'Tense',             icon: '●'  };
    return                           { color: 'var(--text-dim)', label: 'Neutral',   icon: '○'  };
  })();
  return (
    <div style={{
      display: 'flex', alignItems: 'center', gap: '5px',
      fontSize: '10px', color: tone.color, marginTop: '5px', opacity: 0.85,
    }}>
      <span style={{ fontSize: '8px' }}>{tone.icon}</span>
      <span style={{ fontWeight: 600, letterSpacing: '0.04em', textTransform: 'uppercase' }}>
        {tone.label}
      </span>
      {friction.length > 0 && (
        <span style={{ opacity: 0.55, fontStyle: 'italic' }}>"{friction[0]}"</span>
      )}
    </div>
  );
};

// ── SuggestionBar ──────────────────────────────────────────────────────────

const SuggestionBar = ({
  result, selected, onSelect, isRefined,
}: {
  result: IntentResult;
  selected: Mode;
  onSelect: (m: Mode, c: IntentCandidate) => void;
  isRefined: boolean;
}) => {
  const [expanded, setExpanded] = useState(false);

  const confidenceLabel = (c: number) =>
    c > 0.75 ? null : c > 0.50 ? 'likely' : 'maybe';

  const cLabel = confidenceLabel(result.primary.confidence);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '6px', marginBottom: '10px' }}>
      <div style={{ display: 'flex', gap: '6px', alignItems: 'center', flexWrap: 'nowrap', overflowX: 'auto' }}>

        {/* Primary */}
        <button
          onClick={() => onSelect(result.primary.mode, result.primary)}
          className={`mode-pill${selected === result.primary.mode ? ' active' : ''}`}
          title={result.primary.reason}
        >
          {selected !== result.primary.mode && (
            <Sparkles size={10} style={{ opacity: 0.7 }} />
          )}
          {result.primary.label}
          {cLabel && (
            <span style={{ fontSize: '9px', opacity: 0.5, marginLeft: '2px' }}>{cLabel}</span>
          )}
          {isRefined && (
            <span style={{ fontSize: '9px', color: '#60a5fa', marginLeft: '3px' }}>✦</span>
          )}
        </button>

        {/* Alternatives */}
        {result.alternatives.map(alt => (
          <button
            key={alt.intent}
            onClick={() => onSelect(alt.mode, alt)}
            className={`mode-pill${selected === alt.mode ? ' active' : ''}`}
            style={{ opacity: selected === alt.mode ? 1 : 0.5 }}
            title={alt.reason}
          >
            {alt.label}
          </button>
        ))}

        {/* Expand toggle */}
        <button
          onClick={() => setExpanded(v => !v)}
          className="mode-pill"
          style={{ padding: '5px 9px', opacity: 0.4, marginLeft: 'auto' }}
          title="All modes"
        >
          {expanded ? '−' : '···'}
        </button>
      </div>

      {/* Expanded mode grid */}
      <AnimatePresence>
        {expanded && (
          <motion.div
            key="expanded"
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: 'auto' }}
            exit={{ opacity: 0, height: 0 }}
            style={{ overflow: 'hidden' }}
          >
            <div style={{
              display: 'flex', gap: '6px', flexWrap: 'wrap',
              borderTop: '1px solid rgba(255,255,255,0.06)',
              paddingTop: '8px', marginTop: '2px',
            }}>
              {(['Correct', 'Professional', 'Casual', 'Summarize', 'Email', 'Translate', 'Strategist', 'Prompt', 'Custom'] as Mode[]).map(m => (
                <button
                  key={m}
                  className={`mode-pill${selected === m ? ' active' : ''}`}
                  style={{ fontSize: '11px', padding: '4px 9px' }}
                  onClick={() => { onSelect(m, result.primary); setExpanded(false); }}
                >
                  {m}
                </button>
              ))}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};

// ── App ────────────────────────────────────────────────────────────────────

function App() {
  const [capturedText,    setCapturedText]    = useState('')
  const [nlpContext,      setNlpContext]       = useState<TextContext | null>(null)
  const [intentResult,    setIntentResult]     = useState<IntentResult | null>(null)
  const [isRefined,       setIsRefined]        = useState(false)
  const [userInteracted,  setUserInteracted]   = useState(false)

  const [hasKey,          setHasKey]           = useState<boolean | null>(null)
  const [apiKey,          setApiKey]           = useState('')
  const [selectedMode,    setSelectedMode]     = useState<Mode>('Correct')
  const [subIntent,       setSubIntent]        = useState<string | null>(null)
  const [customPrompt,    setCustomPrompt]     = useState('')
  const [streamingResult, setStreamingResult]  = useState('')
  const [isGenerating,    setIsGenerating]     = useState(false)
  const [error,           setError]            = useState('')
  const [useLocal,        setUseLocal]         = useState(false)
  const scrollRef = useRef<HTMLDivElement>(null)

  // ── Event listeners ──────────────────────────────────────────────────────
  useEffect(() => {
    invoke<boolean>('has_api_key').then(setHasKey)

    const unCapture = listen<{ text: string; context: TextContext }>('text_captured', e => {
      const { text, context } = e.payload
      setCapturedText(text)
      setNlpContext(context)
      setIntentResult(context.intent_result)
      setSelectedMode(context.suggested_mode ?? 'Correct')
      setStreamingResult('')
      setError('')
      setIsRefined(false)
      setUserInteracted(false)
    })

    const unRefined = listen<{ intent: string; confidence: number }>('intent_refined', e => {
      if (userInteracted) return
      setIntentResult(prev => prev ? {
        ...prev,
        primary: { ...prev.primary, intent: e.payload.intent, confidence: e.payload.confidence },
      } : prev)
      setIsRefined(true)
      setTimeout(() => setIsRefined(false), 2000)
    })

    const unToken = listen<string>('ai_token', e => {
      setStreamingResult(prev => prev + e.payload)
    })

    const unEnd = listen('ai_stream_end', () => setIsGenerating(false))

    return () => {
      unCapture.then(f => f())
      unRefined.then(f => f())
      unToken.then(f => f())
      unEnd.then(f => f())
    }
  }, [userInteracted])

  // ── Keyboard shortcuts ───────────────────────────────────────────────────
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') handleGenerate()
      else if (e.key === 'Tab')    { e.preventDefault(); handleInsert() }
      else if (e.key === 'Escape') invoke('hide_window')
      else if (e.key === 'c' && !e.ctrlKey && !e.metaKey && streamingResult)
        navigator.clipboard.writeText(streamingResult)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [capturedText, isGenerating, selectedMode, customPrompt, streamingResult])

  // ── Auto-scroll output ───────────────────────────────────────────────────
  useEffect(() => {
    if (scrollRef.current)
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight
  }, [streamingResult])

  // ── Actions ──────────────────────────────────────────────────────────────
  const handleGenerate = async (forcedMode?: Mode) => {
    const mode = forcedMode ?? selectedMode
    if (!capturedText || isGenerating) return
    if (mode === 'Custom' && !customPrompt) return

    setIsGenerating(true)
    setStreamingResult('')
    setError('')

    if (!hasKey || useLocal) {
      try {
        const res = await invoke<string>('generate_local_response', {
          mode, text: capturedText, subMode: subIntent,
        })
        setStreamingResult(res)
      } catch (e: any) {
        setError('Local error: ' + e)
      } finally {
        setIsGenerating(false)
      }
      return
    }

    try {
      await invoke('generate_ai_response', {
        mode, text: capturedText,
        customPrompt: mode === 'Custom' ? customPrompt : null,
        subMode: subIntent,
      })
      // isGenerating reset by ai_stream_end event
    } catch (e: any) {
      setError(e.toString())
      setIsGenerating(false)
    }
  }

  const handleSuggestionClick = (mode: Mode, candidate: IntentCandidate) => {
    setSelectedMode(mode)
    setSubIntent(null)
    setUserInteracted(true)
    if (intentResult && candidate.intent !== intentResult.primary.intent) {
      invoke('record_intent_correction', {
        suggestedIntent: intentResult.primary.intent,
        chosenIntent: candidate.intent,
        confidence: intentResult.primary.confidence,
        textLength: capturedText.length,
      }).catch(console.error)
    }
  }

  const handleInsert = async () => {
    if (!streamingResult || isGenerating) return
    await invoke('inject_result', { text: streamingResult })
  }

  const handleStoreKey = async () => {
    if (!apiKey.trim()) return
    try {
      await invoke('store_api_key', { key: apiKey.trim() })
      setHasKey(true)
    } catch (e: any) {
      setError(e.toString())
    }
  }

  // ── Loading ───────────────────────────────────────────────────────────────
  if (hasKey === null) {
    return (
      <div className="glass-card" style={{ alignItems: 'center', justifyContent: 'center', gap: '12px', minHeight: '160px' }}>
        <Sparkles className="animate-pulse" size={26} color="var(--blue)" />
        <p style={{ fontSize: '13px', color: 'var(--text-muted)' }}>Initializing…</p>
      </div>
    )
  }

  // ── Onboarding ────────────────────────────────────────────────────────────
  if (hasKey === false) {
    return (
      <motion.div
        className="glass-card"
        initial={{ scale: 0.94, opacity: 0, y: 8 }}
        animate={{ scale: 1, opacity: 1, y: 0 }}
        transition={{ type: 'spring', stiffness: 300, damping: 28 }}
        style={{ gap: '0' }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '9px', marginBottom: '14px' }}>
          <Sparkles size={18} color="var(--blue)" />
          <h2 style={{ fontSize: '16px', fontWeight: 600, color: 'var(--text)' }}>Onboarding</h2>
        </div>
        <p style={{ fontSize: '13px', color: 'var(--text-muted)', marginBottom: '16px', lineHeight: 1.65 }}>
          Paste a free Gemini API key from{' '}
          <a
            href="https://aistudio.google.com" target="_blank" rel="noreferrer"
            style={{ color: 'var(--blue)', textDecoration: 'none', borderBottom: '1px solid var(--blue)' }}
          >
            AI Studio
          </a>{' '}
          to activate all AI features.
        </p>
        <input
          type="text"
          placeholder="AIza…"
          value={apiKey}
          onChange={e => setApiKey(e.target.value)}
          onKeyDown={e => e.key === 'Enter' && handleStoreKey()}
          autoFocus
          style={{ marginBottom: '10px' }}
        />
        {error && <p style={{ color: '#ef4444', fontSize: '12px', marginBottom: '10px' }}>{error}</p>}
        <button
          onClick={handleStoreKey}
          className="mode-pill primary-action"
          style={{
            width: '100%', padding: '11px',
            background: apiKey ? 'var(--blue)' : 'rgba(255,255,255,0.05)',
            color: '#fff', fontWeight: 600, fontSize: '13px',
            boxShadow: apiKey ? '0 0 20px var(--blue-glow)' : 'none',
          }}
        >
          Activate Layer
        </button>
        <p style={{ fontSize: '10px', color: 'var(--text-dim)', marginTop: '12px', textAlign: 'center' }}>
          Stored locally in Windows Credential Manager — never leaves your machine
        </p>
      </motion.div>
    )
  }

  // ── Main UI ───────────────────────────────────────────────────────────────
  const canGenerate = !!capturedText && !isGenerating && (selectedMode !== 'Custom' || !!customPrompt)

  return (
    <motion.div
      className="glass-card"
      initial={{ scale: 0.93, opacity: 0, y: 6 }}
      animate={{ scale: 1, opacity: 1, y: 0 }}
      transition={{ type: 'spring', stiffness: 320, damping: 26 }}
    >
      {/* ── Header ─────────────────────────────────────────────── */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '7px' }}>
          <Sparkles size={15} color="var(--blue)" />
          <span style={{ fontWeight: 600, fontSize: '13.5px' }}>AI Overlay</span>
          <button
            onClick={async () => { await invoke('delete_api_key').catch(() => {}); setHasKey(false) }}
            style={{ background: 'none', border: 'none', padding: '0 0 0 2px', cursor: 'pointer', opacity: 0.3, lineHeight: 1 }}
            title="Reset API Key"
          >
            <Settings size={12} color="var(--text)" />
          </button>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <span style={{ fontSize: '10px', color: 'var(--text-dim)' }}>
            {useLocal ? '⚡ Local' : '🔒 Gemini'}
          </span>
          <button
            onClick={() => setUseLocal(v => !v)}
            className="toggle-switch"
            title={useLocal ? 'Switch to AI' : 'Switch to local'}
          >
            <div className={`switch-knob${useLocal ? ' on' : ''}`} />
          </button>
        </div>
      </div>

      {/* ── Captured text + tone ───────────────────────────────── */}
      <div style={{ marginBottom: '10px' }}>
        <div className="text-preview">
          {capturedText
            ? `"${capturedText.slice(0, 120)}${capturedText.length > 120 ? '…' : ''}"`
            : 'Select text anywhere, then press Alt+Shift+S'}
        </div>
        {nlpContext && (
          <TonePill score={nlpContext.tone} friction={nlpContext.friction_phrases} />
        )}
      </div>

      {/* ── Suggestion bar (or fallback static pills) ──────────── */}
      {intentResult ? (
        <SuggestionBar
          result={intentResult}
          selected={selectedMode}
          onSelect={handleSuggestionClick}
          isRefined={isRefined}
        />
      ) : (
        <div style={{ display: 'flex', gap: '6px', flexWrap: 'wrap', marginBottom: '10px' }}>
          {(['Correct', 'Casual', 'Professional', 'Summarize', 'Custom'] as Mode[]).map(m => (
            <button
              key={m}
              className={`mode-pill${selectedMode === m ? ' active' : ''}`}
              onClick={() => setSelectedMode(m)}
            >
              {m}
            </button>
          ))}
        </div>
      )}

      {/* ── Custom prompt input ────────────────────────────────── */}
      <AnimatePresence>
        {selectedMode === 'Custom' && (
          <motion.div
            key="custom"
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: 'auto' }}
            exit={{ opacity: 0, height: 0 }}
            style={{ overflow: 'hidden', marginBottom: '8px' }}
          >
            <input
              type="text"
              placeholder="E.g. 'Make it a tweet' or 'Translate to French'…"
              value={customPrompt}
              onChange={e => setCustomPrompt(e.target.value)}
              autoFocus
            />
          </motion.div>
        )}
      </AnimatePresence>

      {/* ── Output ─────────────────────────────────────────────── */}
      <div className={`token-container${isGenerating ? ' blinking-cursor' : ''}`} ref={scrollRef}>
        {streamingResult
          ? streamingResult
          : isGenerating
            ? 'Thinking…'
            : <span style={{ color: 'var(--text-dim)' }}>Ready for transformation…</span>
        }
        {error && (
          <div style={{ color: '#ef4444', marginTop: '8px', fontSize: '12px' }}>⚠ {error}</div>
        )}
      </div>

      {/* ── Actions ────────────────────────────────────────────── */}
      <div style={{ display: 'flex', gap: '7px', marginTop: '12px' }}>
        <button
          onClick={() => handleGenerate()}
          disabled={!canGenerate}
          className="mode-pill primary-action"
          style={{
            flexGrow: 1, padding: '9px',
            background: canGenerate ? 'var(--blue)' : 'rgba(255,255,255,0.05)',
            color: '#fff', fontWeight: 600, fontSize: '13px',
            boxShadow: canGenerate ? '0 0 18px var(--blue-glow)' : 'none',
          }}
        >
          <Send size={13} />
          {isGenerating ? 'Generating…' : 'Transform'}
        </button>

        <button
          onClick={handleInsert}
          disabled={!streamingResult || isGenerating}
          className="mode-pill"
          title="Insert (Tab)"
          style={{ padding: '9px 12px' }}
        >
          <CheckCircle2 size={15} />
        </button>

        <button
          onClick={() => streamingResult && navigator.clipboard.writeText(streamingResult)}
          disabled={!streamingResult}
          className="mode-pill"
          title="Copy (C)"
          style={{ padding: '9px 12px' }}
        >
          <Copy size={15} />
        </button>
      </div>

      {/* ── Shortcut hints ─────────────────────────────────────── */}
      <div className="shortcut-hints">
        <span>⌘↵ Transform</span>
        <span>Tab Insert</span>
        <span>Esc Dismiss</span>
      </div>
    </motion.div>
  )
}

export default App
