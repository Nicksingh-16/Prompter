import { useState, useEffect, useRef } from 'react'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { motion, AnimatePresence } from 'framer-motion'
import { Settings, Sparkles, Send, CheckCircle2, Copy, Zap, History, X, ChevronRight } from 'lucide-react'
import './index.css'

// ── Types ──────────────────────────────────────────────────────────────────

type Mode =
  | 'Prompt' | 'Summarize' | 'Email' | 'Correct'
  | 'Translate' | 'Casual' | 'Professional'
  | 'Strategist' | 'Knowledge' | 'Custom';

interface IntentCandidate {
  intent: string; confidence: number;
  label: string; mode: Mode; reason: string;
}
interface IntentResult {
  primary: IntentCandidate;
  alternatives: IntentCandidate[];
  overall_confidence: number;
}
interface LanguageContext {
  primary_script: string; primary_pct: number;
  is_mixed: boolean; is_rtl: boolean;
  candidate_languages: string;
}
interface TextContext {
  original: string; normalized: string;
  word_count: number; char_count: number;
  language: LanguageContext; intent_result: IntentResult;
  formality: number; keywords: string[];
  tone: number; friction_phrases: string[];
  suggested_mode: Mode;
}
interface HistoryEntry {
  id: number; timestamp: string;
  input_preview: string; mode: string; output: string;
}

// ── ToneMirror ─────────────────────────────────────────────────────────────

const ToneMirror = ({ score, friction, wordCount, isRtl, isMixed }: {
  score: number; friction: string[];
  wordCount: number; isRtl: boolean; isMixed: boolean;
}) => {
  const verdict = (() => {
    if (friction.length > 0) return {
      color: '#f59e0b', pulse: true,
      line1: 'May read as passive-aggressive',
      line2: `Avoid: "${friction[0]}"`,
    };
    if (isMixed) return {
      color: '#a78bfa', pulse: false,
      line1: 'Hinglish / mixed language detected',
      line2: 'Translate mode suggested',
    };
    if (isRtl) return {
      color: '#a78bfa', pulse: false,
      line1: 'Non-Latin script detected',
      line2: 'Translate mode suggested',
    };
    if (score >= 4) return {
      color: '#10b981', pulse: false,
      line1: 'Reads as warm and enthusiastic',
      line2: wordCount > 80 ? 'Consider trimming for impact' : '',
    };
    if (score >= 1) return {
      color: '#3b82f6', pulse: false,
      line1: 'Reads as clear and professional', line2: '',
    };
    if (score <= -3) return {
      color: '#ef4444', pulse: false,
      line1: 'Reads as frustrated or demanding',
      line2: 'Try Professional mode to de-escalate',
    };
    if (score <= -1) return {
      color: '#f59e0b', pulse: false,
      line1: 'Slightly tense tone', line2: '',
    };
    return { color: 'var(--text-dim)', pulse: false, line1: 'Neutral tone', line2: '' };
  })();

  return (
    <div style={{ marginTop: '6px', display: 'flex', flexDirection: 'column', gap: '2px' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: '7px' }}>
        <span style={{
          width: '6px', height: '6px', borderRadius: '50%',
          background: verdict.color, flexShrink: 0,
          animation: verdict.pulse ? 'tonePulse 1.8s ease-in-out infinite' : 'none',
        }} />
        <span style={{ fontSize: '10.5px', fontWeight: 600, color: verdict.color }}>
          {verdict.line1}
        </span>
      </div>
      {verdict.line2 && (
        <span style={{ fontSize: '9.5px', color: verdict.color, opacity: 0.75, paddingLeft: '13px', fontStyle: 'italic' }}>
          {verdict.line2}
        </span>
      )}
    </div>
  );
};

// ── SuggestionBar ──────────────────────────────────────────────────────────

const ALL_MODES: Mode[] = ['Prompt', 'Correct', 'Translate', 'Summarize', 'Email', 'Professional', 'Casual', 'Strategist', 'Knowledge', 'Custom'];

const SuggestionBar = ({ result, selected, onSelect, isRefined }: {
  result: IntentResult; selected: Mode;
  onSelect: (m: Mode, c: IntentCandidate) => void; isRefined: boolean;
}) => {
  const [expanded, setExpanded] = useState(false);
  const confLabel = (c: number) => c > 0.75 ? null : c > 0.50 ? 'likely' : 'maybe';
  const cLabel = confLabel(result.primary.confidence);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '6px', marginBottom: '10px' }}>
      <div style={{ display: 'flex', gap: '6px', alignItems: 'center', overflowX: 'auto' }}>
        <button
          onClick={() => onSelect(result.primary.mode, result.primary)}
          className={`mode-pill${selected === result.primary.mode ? ' active' : ''}`}
          title={result.primary.reason}
        >
          {selected !== result.primary.mode && <Sparkles size={10} style={{ opacity: 0.7 }} />}
          {result.primary.label}
          {cLabel && <span style={{ fontSize: '9px', opacity: 0.5, marginLeft: '2px' }}>{cLabel}</span>}
          {isRefined && <span style={{ fontSize: '9px', color: '#60a5fa', marginLeft: '3px' }}>✦</span>}
        </button>
        {result.alternatives.slice(0, 2).map(alt => (
          <button key={alt.intent}
            onClick={() => onSelect(alt.mode, alt)}
            className={`mode-pill${selected === alt.mode ? ' active' : ''}`}
            style={{ opacity: selected === alt.mode ? 1 : 0.5 }}
            title={alt.reason}
          >{alt.label}</button>
        ))}
        <button onClick={() => setExpanded(v => !v)} className="mode-pill"
          style={{ padding: '5px 9px', opacity: 0.4, marginLeft: 'auto' }}>
          {expanded ? '−' : '···'}
        </button>
      </div>
      <AnimatePresence>
        {expanded && (
          <motion.div key="expanded"
            initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}
            exit={{ opacity: 0, height: 0 }} style={{ overflow: 'hidden' }}>
            <div style={{
              display: 'flex', gap: '6px', flexWrap: 'wrap',
              borderTop: '1px solid rgba(255,255,255,0.06)', paddingTop: '8px', marginTop: '2px'
            }}>
              {ALL_MODES.map(m => (
                <button key={m}
                  className={`mode-pill${selected === m ? ' active' : ''}`}
                  style={{ fontSize: '11px', padding: '4px 9px' }}
                  onClick={() => { onSelect(m, result.primary); setExpanded(false); }}>
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

// ── History Panel ──────────────────────────────────────────────────────────

const HistoryPanel = ({ onClose, onRestore }: {
  onClose: () => void;
  onRestore: (entry: HistoryEntry) => void;
}) => {
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    invoke<HistoryEntry[]>('get_history', { limit: 20 })
      .then(h => { setEntries(h); setLoading(false); })
      .catch(() => setLoading(false));
  }, []);

  return (
    <motion.div
      initial={{ opacity: 0, x: 20 }}
      animate={{ opacity: 1, x: 0 }}
      exit={{ opacity: 0, x: 20 }}
      style={{
        position: 'absolute', top: 0, left: 0, right: 0, bottom: 0,
        background: 'var(--surface2)', borderRadius: '16px',
        display: 'flex', flexDirection: 'column',
        border: '1px solid var(--border)', zIndex: 10,
      }}
    >
      {/* Header */}
      <div style={{
        display: 'flex', justifyContent: 'space-between', alignItems: 'center',
        padding: '16px 18px 12px', borderBottom: '1px solid rgba(255,255,255,0.05)'
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <History size={14} color="var(--blue)" />
          <span style={{ fontWeight: 600, fontSize: '13px' }}>Recent transforms</span>
        </div>
        <button onClick={onClose} style={{ background: 'none', border: 'none', cursor: 'pointer', opacity: 0.4 }}>
          <X size={14} color="var(--text)" />
        </button>
      </div>

      {/* List */}
      <div style={{ flex: 1, overflowY: 'auto', padding: '8px' }}>
        {loading && (
          <div style={{ textAlign: 'center', padding: '24px', color: 'var(--text-dim)', fontSize: '12px' }}>
            Loading…
          </div>
        )}
        {!loading && entries.length === 0 && (
          <div style={{ textAlign: 'center', padding: '24px', color: 'var(--text-dim)', fontSize: '12px' }}>
            No history yet. Transform some text first.
          </div>
        )}
        {entries.map(entry => (
          <div key={entry.id}
            onClick={() => onRestore(entry)}
            style={{
              padding: '10px 12px', borderRadius: '8px', cursor: 'pointer', marginBottom: '4px',
              background: 'rgba(255,255,255,0.03)', border: '1px solid rgba(255,255,255,0.04)',
              transition: 'background 0.12s',
            }}
            onMouseEnter={e => (e.currentTarget.style.background = 'rgba(255,255,255,0.06)')}
            onMouseLeave={e => (e.currentTarget.style.background = 'rgba(255,255,255,0.03)')}
          >
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '4px' }}>
              <span style={{
                fontSize: '10px', padding: '1px 7px', borderRadius: '4px',
                background: 'rgba(59,130,246,0.15)', color: '#60a5fa', fontWeight: 600,
              }}>{entry.mode}</span>
              <span style={{ fontSize: '9px', color: 'var(--text-dim)' }}>{entry.timestamp.split(' ')[0]}</span>
            </div>
            <div style={{
              fontSize: '11.5px', color: 'var(--text-muted)', whiteSpace: 'nowrap',
              overflow: 'hidden', textOverflow: 'ellipsis', fontStyle: 'italic'
            }}>
              "{entry.input_preview}"
            </div>
            {entry.output && (
              <div style={{ display: 'flex', alignItems: 'center', gap: '4px', marginTop: '4px' }}>
                <ChevronRight size={10} color="var(--text-dim)" />
                <span style={{
                  fontSize: '11px', color: 'var(--text-dim)', whiteSpace: 'nowrap',
                  overflow: 'hidden', textOverflow: 'ellipsis', maxWidth: '320px'
                }}>
                  {entry.output.slice(0, 80)}{entry.output.length > 80 ? '…' : ''}
                </span>
              </div>
            )}
          </div>
        ))}
      </div>
    </motion.div>
  );
};

// ── Onboarding ─────────────────────────────────────────────────────────────

const Onboarding = ({ onComplete, error }: { onComplete: (key: string) => void; error: string }) => {
  const [apiKey, setApiKey] = useState('');
  return (
    <motion.div className="glass-card"
      initial={{ scale: 0.94, opacity: 0, y: 8 }} animate={{ scale: 1, opacity: 1, y: 0 }}
      transition={{ type: 'spring', stiffness: 300, damping: 28 }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: '10px', marginBottom: '20px' }}>
        <div style={{
          width: '36px', height: '36px', borderRadius: '10px',
          background: 'linear-gradient(135deg, #3b82f6, #6366f1)',
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          boxShadow: '0 0 20px rgba(59,130,246,0.4)'
        }}>
          <Sparkles size={18} color="#fff" />
        </div>
        <div>
          <div style={{ fontSize: '15px', fontWeight: 700, color: 'var(--text)' }}>Prompter</div>
          <div style={{ fontSize: '11px', color: 'var(--text-dim)' }}>AI overlay for every app</div>
        </div>
      </div>
      <div style={{
        background: 'rgba(59,130,246,0.08)', border: '1px solid rgba(59,130,246,0.2)',
        borderRadius: '10px', padding: '12px 14px', marginBottom: '18px'
      }}>
        <p style={{ fontSize: '13px', color: 'var(--text)', lineHeight: 1.6, margin: 0 }}>
          Write in <span style={{ color: '#60a5fa', fontWeight: 600 }}>your language, your way</span>.
          Press <kbd style={{
            background: 'rgba(255,255,255,0.1)', border: '1px solid rgba(255,255,255,0.15)',
            borderRadius: '4px', padding: '1px 5px', fontSize: '11px', fontFamily: 'monospace'
          }}>Alt+K</kbd> from
          any app to transform it into professional English instantly.
        </p>
      </div>
      <div style={{ marginBottom: '18px' }}>
        {[
          { n: '1', text: 'Get a free API key from Google AI Studio (30 seconds)' },
          { n: '2', text: 'Paste it below — stored locally, never shared' },
          { n: '3', text: 'Select any text anywhere and press Alt+K' },
        ].map(s => (
          <div key={s.n} style={{ display: 'flex', gap: '10px', alignItems: 'flex-start', marginBottom: '8px' }}>
            <span style={{
              width: '18px', height: '18px', borderRadius: '50%', flexShrink: 0,
              background: 'var(--blue)', display: 'flex', alignItems: 'center', justifyContent: 'center',
              fontSize: '10px', fontWeight: 700, color: '#fff', marginTop: '1px'
            }}>{s.n}</span>
            <span style={{ fontSize: '12px', color: 'var(--text-muted)', lineHeight: 1.5 }}>{s.text}</span>
          </div>
        ))}
      </div>
      <input type="text" placeholder="Paste API key here  (AIza…)" value={apiKey}
        onChange={e => setApiKey(e.target.value)}
        onKeyDown={e => e.key === 'Enter' && apiKey.trim() && onComplete(apiKey.trim())}
        autoFocus style={{ marginBottom: '10px' }} />
      {error && <p style={{ color: '#ef4444', fontSize: '12px', marginBottom: '10px' }}>⚠ {error}</p>}
      <button onClick={() => apiKey.trim() && onComplete(apiKey.trim())}
        className="mode-pill primary-action"
        style={{
          width: '100%', padding: '12px',
          background: apiKey ? 'var(--blue)' : 'rgba(255,255,255,0.05)',
          color: '#fff', fontWeight: 700, fontSize: '14px',
          boxShadow: apiKey ? '0 0 24px var(--blue-glow)' : 'none', borderRadius: '10px'
        }}>
        Activate — it's free
      </button>
      <div style={{ marginTop: '14px', textAlign: 'center' }}>
        <a href="https://aistudio.google.com/app/apikey" target="_blank" rel="noreferrer"
          style={{ fontSize: '11px', color: 'var(--blue)', textDecoration: 'none' }}>
          Get free API key from Google AI Studio →
        </a>
      </div>
      <p style={{ fontSize: '10px', color: 'var(--text-dim)', marginTop: '10px', textAlign: 'center' }}>
        Key stored locally on your device · Never sent to our servers
      </p>
    </motion.div>
  );
};

// ── App ────────────────────────────────────────────────────────────────────

function App() {
  const [capturedText, setCapturedText] = useState('')
  const [nlpContext, setNlpContext] = useState<TextContext | null>(null)
  const [intentResult, setIntentResult] = useState<IntentResult | null>(null)
  const [isRefined, setIsRefined] = useState(false)
  const [userInteracted, setUserInteracted] = useState(false)
  const [hasKey, setHasKey] = useState<boolean | null>(null)
  const [selectedMode, setSelectedMode] = useState<Mode>('Prompt')
  const [subIntent, setSubIntent] = useState<string | null>(null)
  const [customPrompt, setCustomPrompt] = useState('')
  const [streamingResult, setStreamingResult] = useState('')
  const [isGenerating, setIsGenerating] = useState(false)
  const [error, setError] = useState('')
  const [useLocal, setUseLocal] = useState(false)
  const [onboardError, setOnboardError] = useState('')
  const [showHistory, setShowHistory] = useState(false)
  const [usage, setUsage] = useState<{ used: number, cap: number } | null>(null)
  const scrollRef = useRef<HTMLDivElement>(null)

  // ── Event listeners ──────────────────────────────────────────────────────
  useEffect(() => {
    invoke<boolean>('has_api_key').then(setHasKey)

    const unCapture = listen<{ text: string; context: TextContext; force_mode?: string }>('text_captured', e => {
      const { text, context, force_mode } = e.payload
      setCapturedText(text)
      setNlpContext(context)
      setIntentResult(context.intent_result)

      const mode = (force_mode ?? context.suggested_mode ?? 'Prompt') as Mode
      setSelectedMode(mode)
      setStreamingResult('')
      setError('')
      setIsRefined(false)
      setUserInteracted(false)
      setShowHistory(false)

      if (force_mode) {
        setTimeout(() => handleGenerate(force_mode as Mode), 100)
      }

      // Update usage from proxy
      invoke<string>('get_device_id').then(id => {
        fetch(`https://prompter-proxy.onrender.com/usage?device=${id}`)
          .then(r => r.json())
          .then(setUsage)
          .catch(() => { })
      })
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

    const unToken = listen<string>('ai_token', e => setStreamingResult(prev => prev + e.payload))
    const unEnd = listen('ai_stream_end', () => setIsGenerating(false))
    const unError = listen<string>('ai_error', e => { setError(e.payload); setIsGenerating(false); })

    return () => {
      unCapture.then(f => f()); unRefined.then(f => f())
      unToken.then(f => f()); unEnd.then(f => f()); unError.then(f => f())
    }
  }, [userInteracted])

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') handleGenerate()
      else if (e.key === 'Tab') { e.preventDefault(); handleInsert() }
      else if (e.key === 'Escape') {
        if (showHistory) { setShowHistory(false); return; }
        invoke('hide_window')
      }
      else if (e.key === 'c' && !e.ctrlKey && !e.metaKey && streamingResult)
        navigator.clipboard.writeText(streamingResult)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [capturedText, isGenerating, selectedMode, customPrompt, streamingResult, showHistory])

  useEffect(() => {
    if (scrollRef.current) scrollRef.current.scrollTop = scrollRef.current.scrollHeight
  }, [streamingResult])

  // ── Actions ──────────────────────────────────────────────────────────────
  const handleGenerate = async (forcedMode?: Mode) => {
    const mode = forcedMode ?? selectedMode
    if (!capturedText || isGenerating) return
    if (mode === 'Custom' && !customPrompt) return
    setIsGenerating(true); setStreamingResult(''); setError('')

    if (!hasKey || useLocal) {
      try {
        const res = await invoke<string>('generate_local_response', { mode, text: capturedText, subMode: subIntent })
        setStreamingResult(res)
      } catch (e: any) { setError('Local error: ' + e) }
      finally { setIsGenerating(false) }
      return
    }
    try {
      await invoke('generate_ai_response', {
        mode, text: capturedText,
        customPrompt: mode === 'Custom' ? customPrompt : null,
        subMode: subIntent,
      })
    } catch (e: any) { setError(e.toString()); setIsGenerating(false) }
  }

  const handleSuggestionClick = (mode: Mode, candidate: IntentCandidate) => {
    setSelectedMode(mode); setSubIntent(null); setUserInteracted(true)
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
    try { await navigator.clipboard.writeText(streamingResult) } catch { }
    await invoke('inject_result', { text: streamingResult })
  }

  const handleStoreKey = async (key: string) => {
    try { await invoke('store_api_key', { key }); setHasKey(true); setOnboardError('') }
    catch (e: any) { setOnboardError(e.toString()) }
  }

  const handleRestoreHistory = (entry: HistoryEntry) => {
    setCapturedText(entry.input_preview)
    setStreamingResult(entry.output)
    setSelectedMode(entry.mode as Mode)
    setShowHistory(false)
  }

  // ── Loading ───────────────────────────────────────────────────────────────
  if (hasKey === null) return (
    <div className="glass-card" style={{ alignItems: 'center', justifyContent: 'center', gap: '12px', minHeight: '160px' }}>
      <Sparkles className="animate-pulse" size={26} color="var(--blue)" />
      <p style={{ fontSize: '13px', color: 'var(--text-muted)' }}>Starting up…</p>
    </div>
  )

  if (!hasKey) return <Onboarding onComplete={handleStoreKey} error={onboardError} />

  const canGenerate = !!capturedText && !isGenerating && (selectedMode !== 'Custom' || !!customPrompt)
  const isNonLatin = nlpContext && !['Latin', 'Unknown', ''].includes(nlpContext.language.primary_script)
  const isMixed = nlpContext?.language.is_mixed ?? false

  return (
    <motion.div className="glass-card" style={{ position: 'relative' }}
      initial={{ scale: 0.93, opacity: 0, y: 6 }} animate={{ scale: 1, opacity: 1, y: 0 }}
      transition={{ type: 'spring', stiffness: 320, damping: 26 }}>

      {/* ── History panel overlay ───────────────────────────────── */}
      <AnimatePresence>
        {showHistory && (
          <HistoryPanel
            onClose={() => setShowHistory(false)}
            onRestore={handleRestoreHistory}
          />
        )}
      </AnimatePresence>

      {/* ── Header ─────────────────────────────────────────────── */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '7px' }}>
          <Sparkles size={15} color="var(--blue)" />
          <span style={{ fontWeight: 700, fontSize: '13.5px', letterSpacing: '-0.01em' }}>Prompter</span>
          <button onClick={async () => { await invoke('delete_api_key').catch(() => { }); setHasKey(false) }}
            style={{ background: 'none', border: 'none', padding: '0 0 0 2px', cursor: 'pointer', opacity: 0.3 }}
            title="Reset API Key">
            <Settings size={12} color="var(--text)" />
          </button>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          {/* Language badge */}
          {(isNonLatin || isMixed) && (
            <span style={{
              fontSize: '9px', padding: '2px 7px', borderRadius: '10px',
              background: 'rgba(99,102,241,0.15)', border: '1px solid rgba(99,102,241,0.3)',
              color: '#a5b4fc', fontWeight: 600
            }}>
              {isMixed ? 'MIXED' : nlpContext?.language.primary_script.toUpperCase()}
            </span>
          )}
          <button onClick={() => setShowHistory(v => !v)} className="mode-pill"
            style={{ padding: '4px 8px', opacity: showHistory ? 1 : 0.4 }}
            title="Recent transforms (last 20)">
            <History size={12} />
          </button>
          {usage && (
            <span style={{ fontSize: '10px', color: usage.used >= usage.cap ? '#ef4444' : 'var(--text-dim)', fontWeight: 600 }}>
              {usage.used}/{usage.cap}
            </span>
          )}
          <span style={{ fontSize: '10px', color: 'var(--text-dim)' }}>
            {useLocal ? '⚡ Local' : '🔒 Gemini'}
          </span>
          <button onClick={() => setUseLocal(v => !v)} className="toggle-switch">
            <div className={`switch-knob${useLocal ? ' on' : ''}`} />
          </button>
        </div>
      </div>

      {/* ── Captured text + Tone Mirror ────────────────────────── */}
      <div style={{ marginBottom: '10px' }}>
        <div className="text-preview">
          {capturedText
            ? `"${capturedText.slice(0, 120)}${capturedText.length > 120 ? '…' : ''}"`
            : <span style={{ opacity: 0.5 }}>Select text → Alt+K · Prompt → Alt+Shift+K · Fix → Alt+Shift+L</span>
          }
        </div>
        {nlpContext && (
          <ToneMirror
            score={nlpContext.tone}
            friction={nlpContext.friction_phrases}
            wordCount={nlpContext.word_count}
            isRtl={nlpContext.language.is_rtl}
            isMixed={isMixed}
          />
        )}
      </div>

      {/* ── Suggestion bar ─────────────────────────────────────── */}
      {intentResult ? (
        <SuggestionBar result={intentResult} selected={selectedMode}
          onSelect={handleSuggestionClick} isRefined={isRefined} />
      ) : (
        <div style={{ display: 'flex', gap: '6px', flexWrap: 'wrap', marginBottom: '10px' }}>
          {(['Correct', 'Casual', 'Professional', 'Translate', 'Summarize'] as Mode[]).map(m => (
            <button key={m} className={`mode-pill${selectedMode === m ? ' active' : ''}`}
              onClick={() => setSelectedMode(m)}>{m}</button>
          ))}
        </div>
      )}

      {/* ── Custom prompt ──────────────────────────────────────── */}
      <AnimatePresence>
        {selectedMode === 'Custom' && (
          <motion.div key="custom" initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }}
            style={{ overflow: 'hidden', marginBottom: '8px' }}>
            <input type="text" placeholder="E.g. 'Make it a tweet' or 'Translate to French'…"
              value={customPrompt} onChange={e => setCustomPrompt(e.target.value)} autoFocus />
          </motion.div>
        )}
      </AnimatePresence>

      {/* ── Output ─────────────────────────────────────────────── */}
      <div className={`token-container${isGenerating ? ' blinking-cursor' : ''}`} ref={scrollRef}>
        {streamingResult
          ? streamingResult
          : isGenerating ? 'Thinking…'
            : <span style={{ color: 'var(--text-dim)' }}>
              {capturedText ? 'Ready — press Transform or ⌘↵' : 'Waiting for captured text…'}
            </span>
        }
        {error && <div style={{ color: '#ef4444', marginTop: '8px', fontSize: '12px' }}>⚠ {error}</div>}
      </div>

      {/* ── Actions ────────────────────────────────────────────── */}
      <div style={{ display: 'flex', gap: '7px', marginTop: '12px' }}>
        <button onClick={() => handleGenerate()} disabled={!canGenerate}
          className="mode-pill primary-action"
          style={{
            flexGrow: 1, padding: '9px',
            background: canGenerate ? 'var(--blue)' : 'rgba(255,255,255,0.05)',
            color: '#fff', fontWeight: 700, fontSize: '13px',
            boxShadow: canGenerate ? '0 0 18px var(--blue-glow)' : 'none'
          }}>
          {useLocal ? <Zap size={13} /> : <Send size={13} />}
          {isGenerating ? 'Generating…' : 'Transform'}
        </button>
        <button onClick={handleInsert} disabled={!streamingResult || isGenerating}
          className="mode-pill" title="Insert back (Tab)" style={{ padding: '9px 13px' }}>
          <CheckCircle2 size={15} />
        </button>
        <button onClick={() => streamingResult && navigator.clipboard.writeText(streamingResult)}
          disabled={!streamingResult} className="mode-pill" title="Copy (C)" style={{ padding: '9px 13px' }}>
          <Copy size={15} />
        </button>
      </div>

      {/* ── Shortcut hints ─────────────────────────────────────── */}
      <div className="shortcut-hints">
        <span>Alt+K Open</span>
        <span>Alt+⇧+K Prompt</span>
        <span>Alt+⇧+L Fix</span>
        <span>Tab Insert</span>
      </div>
    </motion.div>
  )
}

export default App
