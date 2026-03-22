import { useState, useEffect, useRef } from 'react'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { motion, AnimatePresence } from 'framer-motion'
import { Settings, Sparkles, Send, CheckCircle2, Copy } from 'lucide-react'
import './App.css'

// ── Types ──────────────────────────────────────────────────────────────────

type Mode = 'Prompt' | 'Summarize' | 'Email' | 'Correct' | 'Translate' | 'Casual' | 'Professional' | 'Strategist' | 'Custom';

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

// ── Components ─────────────────────────────────────────────────────────────

const TonePill = ({ score, friction }: { score: number, friction: string[] }) => {
  const getTone = () => {
    if (score >= 3) return { color: '#10b981', label: 'Positive', icon: '✨' };
    if (score <= -2) return { color: '#ef4444', label: 'Negative / Tense', icon: '💢' };
    if (friction.length > 0) return { color: '#f59e0b', label: 'Friction detected', icon: '⚠️' };
    return { color: 'var(--text-dim)', label: 'Neutral', icon: '⚪' };
  };
  const { color, label, icon } = getTone();
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '10px', color, marginTop: '4px', opacity: 0.8 }}>
      <span>{icon}</span>
      <span style={{ fontWeight: 600 }}>{label.toUpperCase()}</span>
      {friction.length > 0 && <span style={{ opacity: 0.6 }}>({friction[0]})</span>}
    </div>
  );
};

const SuggestionBar = ({
  result,
  selected,
  onSelect,
  isRefined
}: {
  result: IntentResult,
  selected: Mode,
  onSelect: (m: Mode, c: IntentCandidate) => void,
  isRefined: boolean
}) => {
  const [expanded, setExpanded] = useState(false);

  const getConfidenceLabel = (c: number) => {
    if (c > 0.75) return null;
    if (c > 0.50) return 'likely';
    return 'maybe';
  };

  const cLabel = getConfidenceLabel(result.primary.confidence);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', marginBottom: '4px' }}>
      <div style={{ display: 'flex', gap: '6px', alignItems: 'center' }}>
        {/* Primary Suggestion */}
        <button
          onClick={() => onSelect(result.primary.mode, result.primary)}
          className={`mode-pill${selected === result.primary.mode ? ' active' : ''}`}
          title={result.primary.reason}
        >
          <Sparkles size={11} style={{ marginRight: '4px', display: selected === result.primary.mode ? 'none' : 'inline' }} />
          {result.primary.label}
          {cLabel && <span style={{ marginLeft: '5px', opacity: 0.5, fontSize: '9px' }}>{cLabel}</span>}
          {isRefined && <span style={{ marginLeft: '5px', color: '#60a5fa', fontSize: '9px' }}>✦ refined</span>}
        </button>

        {/* Alternatives */}
        {result.alternatives.map(alt => (
          <button
            key={alt.intent}
            onClick={() => onSelect(alt.mode, alt)}
            className={`mode-pill${selected === alt.mode ? ' active' : ''}`}
            style={{ opacity: selected === alt.mode ? 1 : 0.4 }}
            title={alt.reason}
          >
            {alt.label}
          </button>
        ))}

        {/* Expand for manual modes */}
        <button
          onClick={() => setExpanded(!expanded)}
          className="mode-pill"
          style={{ padding: '6px 8px', opacity: 0.4 }}
        >
          {expanded ? '−' : '···'}
        </button>
      </div>

      <AnimatePresence>
        {expanded && (
          <motion.div
            initial={{ opacity: 0, y: -5 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -5 }}
            style={{ display: 'flex', gap: '6px', flexWrap: 'wrap', borderTop: '1px solid rgba(255,255,255,0.05)', paddingTop: '8px' }}
          >
            {['Fix', 'Professional', 'Casual', 'Summarize', 'Translate', 'Strategist', 'Custom'].map(m => (
              <button
                key={m}
                className={`mode-pill${selected === m ? ' active' : ''}`}
                style={{ fontSize: '11px', padding: '4px 9px' }}
                onClick={() => onSelect(m as Mode, result.primary /* stub for manual */)}
              >
                {m}
              </button>
            ))}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};

// ── Main App ───────────────────────────────────────────────────────────────

function App() {
  const [capturedText, setCapturedText] = useState('')
  const [nlpContext, setNlpContext] = useState<TextContext | null>(null)
  const [intentResult, setIntentResult] = useState<IntentResult | null>(null)
  const [isRefined, setIsRefined] = useState(false)
  const [userHasInteracted, setUserHasInteracted] = useState(false)

  const [hasKey, setHasKey] = useState<boolean | null>(null)
  const [apiKey, setApiKey] = useState('')
  const [selectedMode, setSelectedMode] = useState<Mode>('Correct')
  const [subIntent, setSubIntent] = useState<string | null>(null)
  const [customPrompt, setCustomPrompt] = useState('')
  const [streamingResult, setStreamingResult] = useState('')
  const [isGenerating, setIsGenerating] = useState(false)
  const [error, setError] = useState('')
  const [useLocalFallback, setUseLocalFallback] = useState(false)
  const scrollRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    invoke<boolean>('has_api_key').then(setHasKey)

    // Text capture payload: { text: string, context: TextContext }
    const unlistenCapture = listen<{ text: string, context: TextContext }>('text_captured', (event) => {
      const { text, context } = event.payload
      setCapturedText(text)
      setNlpContext(context)
      setIntentResult(context.intent_result)
      setSelectedMode(context.suggested_mode || 'Correct')
      setStreamingResult('')
      setError('')
      setIsRefined(false)
      setUserHasInteracted(false)
    })

    // Layer 3: AI Intent Refinement
    const unlistenRefined = listen<{ intent: string, confidence: number, alternatives: any[] }>('intent_refined', (event) => {
      if (userHasInteracted) return; // Don't override user choice

      setIntentResult(prev => {
        if (!prev) return prev;
        // Deep update primary based on AI verdict
        return {
          ...prev,
          primary: {
            ...prev.primary,
            intent: event.payload.intent,
            label: event.payload.intent, // Simplification for now
            confidence: event.payload.confidence
          }
        };
      });
      setIsRefined(true);
      setTimeout(() => setIsRefined(false), 2000);
    })

    // AI token stream
    const unlistenTokens = listen<string>('ai_token', (event) => {
      setStreamingResult(prev => prev + event.payload)
    })

    // Stream end signal
    const unlistenEnd = listen('ai_stream_end', () => {
      setIsGenerating(false)
    })

    return () => {
      unlistenCapture.then(f => f())
      unlistenRefined.then(f => f())
      unlistenTokens.then(f => f())
      unlistenEnd.then(f => f())
    }
  }, [userHasInteracted])

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
        handleGenerate()
      } else if (e.key === 'Tab') {
        e.preventDefault()
        handleInsert()
      } else if (e.key === 'Escape') {
        invoke('hide_window')
      } else if (e.key === 'c' && !e.ctrlKey && !e.metaKey && streamingResult) {
        navigator.clipboard.writeText(streamingResult)
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [capturedText, isGenerating, selectedMode, customPrompt, streamingResult])

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight
    }
  }, [streamingResult])

  const handleGenerate = async (forcedMode?: Mode, forcedSub?: string) => {
    const modeToUse = forcedMode || selectedMode;
    if (!capturedText || isGenerating) return
    if (modeToUse === 'Custom' && !customPrompt) return

    setIsGenerating(true)
    setStreamingResult('')
    setError('')

    if (!hasKey || useLocalFallback) {
      try {
        const localRes = await invoke<string>('generate_local_response', {
          mode: modeToUse,
          text: capturedText,
          subMode: forcedSub || subIntent
        })
        setStreamingResult(localRes)
        setIsGenerating(false)
      } catch (e: any) {
        setError("Local Fallback Error: " + e.toString())
        setIsGenerating(false)
      }
      return
    }

    try {
      await invoke('generate_ai_response', {
        mode: modeToUse,
        text: capturedText,
        customPrompt: modeToUse === 'Custom' ? customPrompt : null,
        subMode: forcedSub || subIntent
      })
    } catch (e: any) {
      setError(e.toString())
      setIsGenerating(false)
    }
  }

  const handleSuggestionClick = (mode: Mode, candidate: IntentCandidate) => {
    setSelectedMode(mode);
    setSubIntent(null);
    setUserHasInteracted(true);

    // Layer 4: Save correction if user changed from primary
    if (intentResult && candidate.intent !== intentResult.primary.intent) {
      invoke('record_intent_correction', {
        suggestedIntent: intentResult.primary.intent,
        chosenIntent: candidate.intent,
        confidence: intentResult.primary.confidence,
        textLength: capturedText.length
      }).catch(console.error);
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

  if (hasKey === null) {
    return (
      <div className="glass-card" style={{ alignItems: 'center', justifyContent: 'center', gap: '12px' }}>
        <Sparkles className="animate-pulse" size={28} color="var(--blue)" />
        <p style={{ margin: 0, fontSize: '13px', color: 'var(--text-muted)' }}>Initializing…</p>
      </div>
    )
  }

  if (hasKey === false) {
    return (
      <motion.div
        className="glass-card"
        initial={{ scale: 0.94, opacity: 0, y: 8 }}
        animate={{ scale: 1, opacity: 1, y: 0 }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '9px', marginBottom: '14px' }}>
          <Sparkles size={19} color="var(--blue)" />
          <h2 style={{ fontSize: '17px', margin: 0, fontWeight: 600 }}>Onboarding</h2>
        </div>
        <p style={{ fontSize: '13px', color: 'var(--text-muted)', marginBottom: '18px', lineHeight: 1.6 }}>
          Welcome! Paste a free Gemini API key from{' '}
          <a href="https://aistudio.google.com" target="_blank" rel="noreferrer" style={{ color: 'var(--blue)', textDecoration: 'none', borderBottom: '1px solid var(--blue)' }}>AI Studio</a> to get started.
        </p>
        <input
          type="text"
          placeholder="AIza…"
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && handleStoreKey()}
          autoFocus
        />
        <button onClick={handleStoreKey} className="mode-pill" style={{ width: '100%', padding: '11px', background: apiKey ? 'var(--blue)' : 'rgba(255,255,255,0.05)', color: '#fff', fontWeight: 600, fontSize: '13px' }}>
          Activate Layer
        </button>
      </motion.div>
    )
  }

  const canGenerate = !!capturedText && !isGenerating && (selectedMode !== 'Custom' || !!customPrompt)

  return (
    <motion.div
      className="glass-card"
      initial={{ scale: 0.92, opacity: 0, y: 6 }}
      animate={{ scale: 1, opacity: 1, y: 0 }}
    >
      {/* Header */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '10px' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '7px' }}>
          <Sparkles size={16} color="var(--blue)" />
          <span style={{ fontWeight: 600, fontSize: '14px' }}>AI Overlay</span>
          <button onClick={async () => { await invoke('delete_api_key').catch(() => { }); setHasKey(false); }} style={{ background: 'none', border: 'none', opacity: 0.3 }} title="Reset Key">
            <Settings size={12} color="#fff" />
          </button>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <span style={{ fontSize: '10px', color: 'var(--text-dim)' }}>
            {useLocalFallback ? '⚡ Local' : '🔒 Gemini 1.5 Flash'}
          </span>
          <button onClick={() => setUseLocalFallback(!useLocalFallback)} className="toggle-switch">
            <div className={`switch-knob ${useLocalFallback ? 'on' : ''}`} />
          </button>
        </div>
      </div>

      {/* Captured text preview + TonePill */}
      <div style={{ marginBottom: '12px' }}>
        <div className="text-preview">
          {capturedText
            ? `"${capturedText.slice(0, 110)}${capturedText.length > 110 ? '…' : ''}"`
            : 'Capture text with Alt+K'}
        </div>
        {nlpContext && (
          <TonePill score={nlpContext.tone} friction={nlpContext.friction_phrases} />
        )}
      </div>

      {/* Suggestion Bar */}
      {intentResult && (
        <SuggestionBar
          result={intentResult}
          selected={selectedMode}
          onSelect={handleSuggestionClick}
          isRefined={isRefined}
        />
      )}

      {/* Custom prompt input */}
      <AnimatePresence>
        {selectedMode === 'Custom' && (
          <motion.div key="custom" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }} style={{ overflow: 'hidden', marginTop: '8px' }}>
            <input
              type="text"
              placeholder="E.g. 'Shorten this' or 'Formal tone'…"
              value={customPrompt}
              onChange={(e) => setCustomPrompt(e.target.value)}
              autoFocus
            />
          </motion.div>
        )}
      </AnimatePresence>

      {/* Output */}
      <div className={`token-container${isGenerating ? ' blinking-cursor' : ''}`} ref={scrollRef}>
        {streamingResult || (isGenerating ? 'Thinking…' : <span style={{ color: 'var(--text-dim)' }}>Ready for transformation…</span>)}
        {error && <div style={{ color: '#ef4444', marginTop: '8px', fontSize: '12px' }}>⚠ {error}</div>}
      </div>

      {/* Actions */}
      <div style={{ display: 'flex', gap: '8px', marginTop: '14px' }}>
        <button
          onClick={() => handleGenerate()}
          disabled={!canGenerate}
          className="mode-pill primary-action"
          style={{ flexGrow: 1, background: canGenerate ? 'var(--blue)' : 'rgba(255,255,255,0.05)', boxShadow: canGenerate ? '0 0 18px var(--blue-glow)' : 'none' }}
        >
          <Send size={13} />
          {isGenerating ? 'Generating…' : 'Transform'}
        </button>

        <button onClick={handleInsert} disabled={!streamingResult || isGenerating} className="mode-pill" title="Insert (Tab)">
          <CheckCircle2 size={15} />
        </button>

        <button onClick={() => streamingResult && navigator.clipboard.writeText(streamingResult)} disabled={!streamingResult} className="mode-pill" title="Copy (C)">
          <Copy size={15} />
        </button>
      </div>

      <div className="shortcut-hints">
        <span>⌘↵ Transform</span>
        <span>Tab Insert</span>
        <span>Esc Dismiss</span>
      </div>
    </motion.div>
  )
}

export default App
