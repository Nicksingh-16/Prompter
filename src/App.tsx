import { useState, useEffect, useRef, useCallback } from 'react'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { motion, AnimatePresence } from 'framer-motion'
import { Settings, Sparkles, Send, CheckCircle2, Copy, Check, Zap, History, X, ChevronRight } from 'lucide-react'
import './index.css'

// ── Constants ──────────────────────────────────────────────────────────────

const FREE_DAILY_CAP = 20

// Priority 2: Primary modes always visible, rest behind ···
const PRIMARY_MODES: Mode[] = ['Reply', 'Do', 'Correct', 'Prompt']
const HIDDEN_MODES: Mode[] = ['Translate', 'Email', 'Summarize', 'Casual', 'Strategist', 'Knowledge', 'Custom']

// ── Types ──────────────────────────────────────────────────────────────────

type Mode =
  | 'Reply' | 'Do' | 'Correct' | 'Prompt'
  | 'Translate' | 'Email' | 'Summarize' | 'Casual'
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

// ── SuggestionBar — Priority 2 ─────────────────────────────────────────────
// Always shows Prompt, Correct, Translate as the 3 fixed pills.
// NLP suggestion highlighted with sparkle + active state, not reordered.
// Everything else lives behind ···

const SuggestionBar = ({ result, selected, onSelect, isRefined }: {
  result: IntentResult; selected: Mode;
  onSelect: (m: Mode, c: IntentCandidate) => void; isRefined: boolean;
}) => {
  const [expanded, setExpanded] = useState(false);

  const getCandidateForMode = (mode: Mode): IntentCandidate => {
    if (result.primary.mode === mode) return result.primary;
    const alt = result.alternatives.find(a => a.mode === mode);
    if (alt) return alt;
    return { intent: mode, confidence: 0, label: mode, mode, reason: '' };
  };

  const isPrimary = (mode: Mode) => result.primary.mode === mode;
  const confLabel = (c: number) => c > 0.75 ? null : c > 0.50 ? 'likely' : 'maybe';

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '6px', marginBottom: '10px' }}>
      <div style={{ display: 'flex', gap: '6px', alignItems: 'center' }}>
        {PRIMARY_MODES.map(mode => {
          const candidate = getCandidateForMode(mode);
          const isActive = selected === mode;
          const isNlpSuggested = isPrimary(mode);
          const cLabel = isNlpSuggested ? confLabel(result.primary.confidence) : null;
          return (
            <button
              key={mode}
              onClick={() => onSelect(mode, candidate)}
              className={`mode-pill${isActive ? ' active' : ''}`}
              style={{ opacity: isActive ? 1 : isNlpSuggested ? 0.9 : 0.55 }}
              title={candidate.reason || mode}
            >
              {isNlpSuggested && !isActive && <Sparkles size={10} style={{ opacity: 0.7 }} />}
              {mode}
              {cLabel && <span style={{ fontSize: '9px', opacity: 0.5, marginLeft: '2px' }}>{cLabel}</span>}
              {isRefined && isNlpSuggested && <span style={{ fontSize: '9px', color: '#60a5fa', marginLeft: '3px' }}>✦</span>}
            </button>
          );
        })}
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
              {HIDDEN_MODES.map(m => (
                <button key={m}
                  className={`mode-pill${selected === m ? ' active' : ''}`}
                  style={{ fontSize: '11px', padding: '4px 9px' }}
                  onClick={() => { onSelect(m, getCandidateForMode(m)); setExpanded(false); }}>
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
  const [query, setQuery] = useState('');

  useEffect(() => {
    invoke<HistoryEntry[]>('get_history', { limit: 50 })
      .then(h => { setEntries(h); setLoading(false); })
      .catch(() => setLoading(false));
  }, []);

  const q = query.toLowerCase().trim();
  const visible = q
    ? entries.filter(e =>
        e.input_preview.toLowerCase().includes(q) ||
        (e.output ?? '').toLowerCase().includes(q) ||
        e.mode.toLowerCase().includes(q)
      )
    : entries;

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
      <div style={{ padding: '8px 12px 4px' }}>
        <input
          autoFocus
          placeholder="Search history…"
          value={query}
          onChange={e => setQuery(e.target.value)}
          style={{
            width: '100%', boxSizing: 'border-box',
            background: 'rgba(255,255,255,0.05)', border: '1px solid rgba(255,255,255,0.1)',
            borderRadius: '7px', padding: '6px 10px', fontSize: '12px',
            color: 'var(--text)', outline: 'none',
          }}
        />
      </div>
      <div style={{ flex: 1, overflowY: 'auto', padding: '8px' }}>
        {loading && (
          <div style={{ textAlign: 'center', padding: '24px', color: 'var(--text-dim)', fontSize: '12px' }}>
            Loading…
          </div>
        )}
        {!loading && visible.length === 0 && (
          <div style={{ textAlign: 'center', padding: '24px', color: 'var(--text-dim)', fontSize: '12px' }}>
            {q ? 'No matches.' : 'No history yet. Transform some text first.'}
          </div>
        )}
        {visible.map(entry => (
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

// ── Settings Modal ─────────────────────────────────────────────────────────

interface CommReport {
  avg_tone: number;
  avg_formality: number;
  total_sessions: number;
  frequent_entities: string[];
  friction_hotspots: string[];
}

const SettingsModal = ({ onClose, onKeySave }: {
  onClose: () => void,
  onKeySave: (key: string) => void
}) => {
  const [mode, setMode] = useState<'Worker' | 'Byok' | 'Local'>('Worker');
  const [key, setKey] = useState('');
  const [stats, setStats] = useState<{ ram_gb: number, cpu_count: number } | null>(null);
  const [report, setReport] = useState<CommReport | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    Promise.all([
      invoke<string>('get_ai_mode'),
      invoke<any>('get_hardware_stats'),
      invoke<string>('get_config_value', { key: 'byok_key' }).catch(() => ''),
      invoke<CommReport>('get_communication_score').catch(() => null),
    ]).then(([m, s, k, r]) => {
      setMode(m.replace(/"/g, '') as any);
      setStats(s);
      setKey(k);
      setReport(r);
      setLoading(false);
    });
  }, []);

  const save = async () => {
    await invoke('set_ai_mode', { mode });
    if (mode === 'Byok') {
      await invoke('store_api_key', { key });
      await invoke('set_config_value', { key: 'byok_key', value: key });
    }
    onKeySave(key);
    onClose();
  };

  return (
    <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}
      style={{
        position: 'absolute', top: 0, left: 0, right: 0, bottom: 0,
        background: 'rgba(0,0,0,0.6)', backdropFilter: 'blur(8px)',
        zIndex: 100, display: 'flex', alignItems: 'center', justifyContent: 'center', padding: '20px'
      }}>
      <motion.div initial={{ scale: 0.9, y: 10 }} animate={{ scale: 1, y: 0 }}
        style={{
          background: 'var(--surface2)', width: '100%', maxWidth: '340px',
          borderRadius: '20px', border: '1px solid var(--border)', padding: '20px',
          boxShadow: '0 20px 40px rgba(0,0,0,0.4)'
        }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '20px' }}>
          <h3 style={{ margin: 0, fontSize: '15px', fontWeight: 700 }}>AI Settings</h3>
          <button onClick={onClose} style={{ background: 'none', border: 'none', cursor: 'pointer', opacity: 0.5 }}>
            <X size={18} />
          </button>
        </div>

        {loading ? <div style={{ textAlign: 'center', padding: '20px' }}>Loading…</div> : (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
            <div>
              <label style={{ fontSize: '11px', color: 'var(--text-dim)', marginBottom: '8px', display: 'block' }}>INFERENCE ENGINE</label>
              <div style={{ display: 'flex', gap: '4px', background: 'rgba(0,0,0,0.2)', padding: '3px', borderRadius: '10px' }}>
                {(['Worker', 'Byok', 'Local'] as const).map(m => (
                  <button key={m} onClick={() => setMode(m)}
                    style={{
                      flex: 1, padding: '7px 0', border: 'none', borderRadius: '7px', fontSize: '11px', fontWeight: 600,
                      background: mode === m ? 'var(--blue)' : 'transparent',
                      color: mode === m ? '#fff' : 'var(--text-dim)', cursor: 'pointer', transition: 'all 0.2s'
                    }}>
                    {m === 'Worker' ? 'Cloud' : m === 'Byok' ? 'BYOK' : 'Local'}
                  </button>
                ))}
              </div>
            </div>

            {mode === 'Byok' && (
              <motion.div initial={{ opacity: 0, y: -5 }} animate={{ opacity: 1, y: 0 }}>
                <label style={{ fontSize: '11px', color: 'var(--text-dim)', marginBottom: '8px', display: 'block' }}>GEMINI API KEY</label>
                <input type="password" placeholder="AIza..." value={key} onChange={e => setKey(e.target.value)}
                  style={{ width: '100%', background: 'rgba(0,0,0,0.2)', border: '1px solid var(--border)', borderRadius: '8px', padding: '10px', color: '#fff', fontSize: '12px' }} />
              </motion.div>
            )}

            {mode === 'Local' && (
              <div style={{ background: 'rgba(59,130,246,0.05)', padding: '10px', borderRadius: '10px', border: '1px solid rgba(59,130,246,0.1)' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '4px' }}>
                  <Zap size={14} color="var(--blue)" />
                  <span style={{ fontSize: '12px', fontWeight: 600 }}>Ollama Mode</span>
                </div>
                <p style={{ fontSize: '10.5px', color: 'var(--text-dim)', margin: 0, lineHeight: 1.4 }}>
                  Uses your local GPU/CPU. Make sure Ollama is running with <code>phi3</code> or <code>gemma2</code>.
                </p>
              </div>
            )}

            <div style={{ display: 'flex', justifyContent: 'space-between', padding: '10px 0', borderTop: '1px solid rgba(255,255,255,0.05)' }}>
              <span style={{ fontSize: '11px', color: 'var(--text-dim)' }}>Hardware</span>
              <span style={{ fontSize: '11px', color: 'var(--text)', fontWeight: 600 }}>
                {stats ? `${stats.cpu_count} CPU · ${stats.ram_gb}GB RAM` : '...'}
              </span>
            </div>

            {report && report.total_sessions > 0 && (
              <div style={{
                background: 'rgba(59,130,246,0.05)', borderRadius: '10px',
                border: '1px solid rgba(59,130,246,0.1)', padding: '12px',
              }}>
                <div style={{ fontSize: '10px', color: 'var(--text-dim)', marginBottom: '8px', fontWeight: 600, letterSpacing: '0.05em' }}>
                  7-DAY COMMUNICATION INSIGHTS
                </div>
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '8px' }}>
                  <div>
                    <div style={{ fontSize: '18px', fontWeight: 700, color: 'var(--text)' }}>{report.total_sessions}</div>
                    <div style={{ fontSize: '10px', color: 'var(--text-dim)' }}>transforms</div>
                  </div>
                  <div>
                    <div style={{ fontSize: '18px', fontWeight: 700, color: report.avg_tone >= 1 ? '#10b981' : report.avg_tone <= -1 ? '#ef4444' : 'var(--text)' }}>
                      {report.avg_tone > 0 ? '+' : ''}{report.avg_tone.toFixed(1)}
                    </div>
                    <div style={{ fontSize: '10px', color: 'var(--text-dim)' }}>avg tone</div>
                  </div>
                </div>
                {report.frequent_entities.length > 0 && (
                  <div style={{ marginTop: '8px', fontSize: '10.5px', color: 'var(--text-muted)' }}>
                    Top contacts: {report.frequent_entities.slice(0, 3).join(', ')}
                  </div>
                )}
                {report.friction_hotspots.length > 0 && (
                  <div style={{ marginTop: '4px', fontSize: '10px', color: '#f59e0b' }}>
                    ⚡ Friction with: {report.friction_hotspots.join(', ')}
                  </div>
                )}
              </div>
            )}

            <button onClick={save} className="mode-pill primary-action"
              style={{ width: '100%', padding: '12px', background: 'var(--blue)', color: '#fff', fontWeight: 700, borderRadius: '10px', marginTop: '4px' }}>
              Save Configuration
            </button>

            <p style={{ fontSize: '10px', color: 'var(--text-dim)', margin: '4px 0 0', lineHeight: 1.5 }}>
              A random device ID is sent with each request to enforce daily usage limits.
              It contains no personal information and cannot identify you.
            </p>
          </div>
        )}
      </motion.div>
    </motion.div>
  );
};

// ── Priority 4: First-run screen ───────────────────────────────────────────
// Shown exactly once. Stored in SQLite config as first_run_done.
// No API key required — proxy mode handles everything.

const FirstRun = ({ onDone }: { onDone: () => void }) => (
  <motion.div className="glass-card"
    initial={{ scale: 0.95, opacity: 0, y: 10 }} animate={{ scale: 1, opacity: 1, y: 0 }}
    transition={{ duration: 0.4, ease: [0.22, 1, 0.36, 1] }}>

    <div style={{ display: 'flex', alignItems: 'center', gap: '10px', marginBottom: '24px' }}>
      <div style={{
        width: '40px', height: '40px', borderRadius: '12px',
        background: 'linear-gradient(135deg, #3b82f6, #6366f1)',
        display: 'flex', alignItems: 'center', justifyContent: 'center',
        boxShadow: '0 0 24px rgba(59,130,246,0.4)', flexShrink: 0,
      }}>
        <Sparkles size={20} color="#fff" />
      </div>
      <div>
        <div style={{ fontSize: '17px', fontWeight: 700, color: 'var(--text)', letterSpacing: '-0.02em' }}>SnapText</div>
        <div style={{ fontSize: '11px', color: 'var(--text-dim)' }}>AI overlay for every app</div>
      </div>
    </div>

    <div style={{
      background: 'rgba(59,130,246,0.08)', border: '1px solid rgba(59,130,246,0.18)',
      borderRadius: '12px', padding: '16px 18px', marginBottom: '22px',
    }}>
      <p style={{ fontSize: '13.5px', color: 'var(--text)', lineHeight: 1.65, margin: 0, fontWeight: 500 }}>
        Select rough text anywhere, press{' '}
        <kbd style={{
          background: 'rgba(255,255,255,0.1)', border: '1px solid rgba(255,255,255,0.18)',
          borderRadius: '5px', padding: '2px 7px', fontSize: '12px',
          fontFamily: 'monospace', color: '#93c5fd',
        }}>Alt+K</kbd>
        {' '}— get a structured AI prompt instantly.
      </p>
    </div>

    <div style={{ display: 'flex', flexDirection: 'column', gap: '12px', marginBottom: '24px' }}>
      {[
        { hotkey: 'Ctrl+C ×2', desc: 'Copy twice — pill appears to transform instantly' },
        { hotkey: 'Alt+K', desc: 'Open full overlay — preview before inserting' },
        { hotkey: 'Alt+⇧+K', desc: 'Silent: transform as a structured AI prompt' },
        { hotkey: 'Alt+⇧+L', desc: 'Silent: fix grammar, spelling, or translate' },
      ].map(({ hotkey, desc }) => (
        <div key={hotkey} style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
          <kbd style={{
            background: 'rgba(255,255,255,0.07)', border: '1px solid rgba(255,255,255,0.12)',
            borderRadius: '6px', padding: '3px 9px', fontSize: '11px',
            fontFamily: 'monospace', color: 'var(--text)', whiteSpace: 'nowrap',
            flexShrink: 0, minWidth: '80px', textAlign: 'center',
          }}>{hotkey}</kbd>
          <span style={{ fontSize: '12px', color: 'var(--text-muted)', lineHeight: 1.4 }}>{desc}</span>
        </div>
      ))}
    </div>

    <button
      onClick={onDone}
      className="mode-pill primary-action"
      style={{
        width: '100%', padding: '13px',
        background: 'var(--blue)', color: '#fff',
        fontWeight: 700, fontSize: '14px', borderRadius: '10px',
        boxShadow: '0 0 24px var(--blue-glow)',
      }}
    >
      Got it — let's go
    </button>

    <p style={{ fontSize: '10px', color: 'var(--text-dim)', marginTop: '12px', textAlign: 'center' }}>
      20 free transforms/day · No account needed
    </p>
  </motion.div>
);

// ── App ────────────────────────────────────────────────────────────────────

function App() {
  const [capturedText, setCapturedText] = useState('')
  const [nlpContext, setNlpContext] = useState<TextContext | null>(null)
  const [intentResult, setIntentResult] = useState<IntentResult | null>(null)
  const [isRefined, setIsRefined] = useState(false)
  const [userInteracted, setUserInteracted] = useState(false)
  const [hasKey, setHasKey] = useState<boolean | null>(null)
  const [firstRunDone, setFirstRunDone] = useState<boolean | null>(null) // null=loading
  const [selectedMode, setSelectedMode] = useState<Mode>('Prompt')
  const [customPrompt, setCustomPrompt] = useState('')
  const [streamingResult, setStreamingResult] = useState('')
  const [isGenerating, setIsGenerating] = useState(false)
  const [error, setError] = useState('')
  const [showHistory, setShowHistory] = useState(false)
  const [showSettings, setShowSettings] = useState(false)
  const [copied, setCopied] = useState(false)
  const [sensitiveNotice, setSensitiveNotice] = useState('')
  const [usage, setUsage] = useState<{ used: number; cap: number }>({ used: 0, cap: FREE_DAILY_CAP })
  const [aiMode, setAiMode] = useState<string>('Worker')
  const [appContext, setAppContext] = useState<string | null>(null)
  const scrollRef = useRef<HTMLDivElement>(null)
  const pendingAutoGenerate = useRef<Mode | null>(null)
  const lastGenerateTime = useRef(0)
  const autoCloseTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  // Refs for stale-closure-safe access inside event listeners
  const selectedModeRef    = useRef<Mode>('Prompt')
  const capturedTextRef    = useRef('')
  const streamingResultRef = useRef('')
  // Tracks the last Reply generation not yet confirmed as accepted/rejected
  const pendingReplyFeedback = useRef<{ input: string; aiOutput: string } | null>(null)

  /** Race a promise against a timeout — prevents UI freeze if backend hangs. */
  const withTimeout = <T,>(promise: Promise<T>, ms: number): Promise<T> => {
    const timeout = new Promise<never>((_, reject) =>
      setTimeout(() => reject(new Error(`Request timed out after ${ms / 1000}s`)), ms)
    )
    return Promise.race([promise, timeout])
  }

  // Auto-generate: fires after state settles from text_captured
  useEffect(() => {
    if (pendingAutoGenerate.current && capturedText && !isGenerating) {
      const mode = pendingAutoGenerate.current
      pendingAutoGenerate.current = null
      handleGenerate(mode)
    }
  }, [capturedText])

  // ── Usage helper ─────────────────────────────────────────────────────────
  const refreshUsage = async () => {
    try {
      const result = await invoke<{ used: number; cap: number } | null>('get_worker_usage').catch(() => null)
      if (result) setUsage({ used: result.used, cap: result.cap })
    } catch { /* Worker offline — keep defaults */ }
  }

  // ── Boot ─────────────────────────────────────────────────────────────────
  useEffect(() => {
    // Safety timeout: if invokes don't resolve in 4s, assume defaults so
    // the app never gets stuck on "Starting up…" (e.g. on a fresh PC).
    const bootTimeout = setTimeout(() => {
      setHasKey(prev => prev ?? true)        // assume Worker mode (no key needed)
      setFirstRunDone(prev => prev ?? false)  // show first-run screen
    }, 4000)

    invoke<boolean>('has_api_key')
      .then(setHasKey)
      .catch(() => setHasKey(true))

    invoke<string>('get_ai_mode')
      .then(m => setAiMode(m.replace(/"/g, '')))
      .catch(() => {})

    // Priority 4: check first_run_done in SQLite config
    invoke<string>('get_config_value', { key: 'first_run_done' })
      .then(() => setFirstRunDone(true))
      .catch(() => setFirstRunDone(false)) // missing key = first run

    // Priority 3: fetch usage immediately on app start
    refreshUsage()

    return () => clearTimeout(bootTimeout)
  }, [])

  // ── Event listeners ──────────────────────────────────────────────────────
  useEffect(() => {
    const unlisteners: (() => void)[] = []

    const setup = async () => {
      unlisteners.push(await listen<{ text: string; context: TextContext; forced_mode?: string; app_context?: string }>('text_captured', e => {
        const { text, context, forced_mode, app_context } = e.payload

        // New session starting — if previous Reply was generated but never injected, mark rejected
        if (pendingReplyFeedback.current) {
          invoke('record_reply_feedback', {
            input:       pendingReplyFeedback.current.input,
            aiOutput:    pendingReplyFeedback.current.aiOutput,
            accepted:    false,
            contactHint: null,
          }).catch(() => {})
          pendingReplyFeedback.current = null
        }

        setCapturedText(text)
        setNlpContext(context)
        setIntentResult(context.intent_result)
        setAppContext(app_context ?? null)

        const mode = (forced_mode ?? context.suggested_mode ?? 'Prompt') as Mode
        setSelectedMode(mode)
        setStreamingResult('')
        setError('')
        setSensitiveNotice('')
        setIsRefined(false)
        setUserInteracted(false)
        setShowHistory(false)
        setCopied(false)

        // Cancel any pending auto-close
        if (autoCloseTimer.current) { clearTimeout(autoCloseTimer.current); autoCloseTimer.current = null }

        // Auto-generate: for Prompt mode OR when triggered from pill
        if ((mode === 'Prompt' || forced_mode) && text.trim()) {
          pendingAutoGenerate.current = mode
        }

        // Priority 3: refresh usage when overlay opens
        refreshUsage()
      }))

      unlisteners.push(await listen<{ intent: string; confidence: number }>('intent_refined', e => {
        if (userInteracted) return
        setIntentResult(prev => prev ? {
          ...prev,
          primary: { ...prev.primary, intent: e.payload.intent, confidence: e.payload.confidence },
        } : prev)
        setIsRefined(true)
        setTimeout(() => setIsRefined(false), 2000)
      }))

      unlisteners.push(await listen<string>('ai_token', e => setStreamingResult(prev => prev + e.payload)))
      unlisteners.push(await listen('ai_stream_end', () => {
        setIsGenerating(false)
        refreshUsage()
        // Track Reply generation for feedback — refs are always fresh (no stale closure)
        if (selectedModeRef.current === 'Reply' && streamingResultRef.current.trim()) {
          pendingReplyFeedback.current = {
            input:    capturedTextRef.current,
            aiOutput: streamingResultRef.current,
          }
        }
        // Auto-close overlay 8s after generation completes (user can Tab/copy before then)
        if (autoCloseTimer.current) clearTimeout(autoCloseTimer.current)
        autoCloseTimer.current = setTimeout(() => invoke('hide_window'), 8000)
      }))
      unlisteners.push(await listen<string>('ai_error', e => { setError(e.payload); setIsGenerating(false) }))
      unlisteners.push(await listen<string>('sensitive_data_detected', e => {
        setSensitiveNotice(`History not saved (${e.payload})`)
        setTimeout(() => setSensitiveNotice(''), 4000)
      }))
    }

    setup()
    return () => { unlisteners.forEach(fn => fn()) }
  }, [userInteracted])

  // ── Actions ──────────────────────────────────────────────────────────────
  const handleGenerate = useCallback(async (forcedMode?: Mode) => {
    // 500ms debounce — prevents duplicate fires before isGenerating is set
    const now = Date.now()
    if (now - lastGenerateTime.current < 500) return
    lastGenerateTime.current = now

    const mode = forcedMode ?? selectedMode
    if (!capturedText || isGenerating) return
    if (mode === 'Custom' && !customPrompt) return
    setIsGenerating(true); setStreamingResult(''); setError(''); setCopied(false)

    try {
      await withTimeout(
        invoke<string>('generate_ai_response', {
          mode, text: capturedText,
          customPrompt: mode === 'Custom' ? customPrompt : null,
          subMode: null,
        }),
        45_000
      )
      // Tokens and stream end handled by event listeners
    } catch (e: any) {
      setError(e.toString())
      setIsGenerating(false)
    }
  }, [capturedText, isGenerating, selectedMode, customPrompt, hasKey, aiMode])

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') handleGenerate()
      else if (e.key === 'Tab') { e.preventDefault(); handleInsert() }
      else if (e.key === 'Escape') {
        if (showHistory) { setShowHistory(false); return; }
        invoke('hide_window')
      }
      else if (e.key === 'c' && !e.ctrlKey && !e.metaKey && streamingResult) {
        navigator.clipboard.writeText(streamingResult).then(() => {
          setCopied(true); setTimeout(() => setCopied(false), 1500)
        }).catch(() => {})
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [handleGenerate, streamingResult, showHistory])

  useEffect(() => {
    if (scrollRef.current) scrollRef.current.scrollTop = scrollRef.current.scrollHeight
  }, [streamingResult])

  // Keep refs in sync so event-listener closures always see fresh values
  useEffect(() => { selectedModeRef.current    = selectedMode    }, [selectedMode])
  useEffect(() => { capturedTextRef.current    = capturedText    }, [capturedText])
  useEffect(() => { streamingResultRef.current = streamingResult }, [streamingResult])

  const handleSuggestionClick = (mode: Mode, candidate: IntentCandidate) => {
    setSelectedMode(mode); setUserInteracted(true)
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
    if (autoCloseTimer.current) { clearTimeout(autoCloseTimer.current); autoCloseTimer.current = null }
    // Record accepted feedback for Reply mode before injecting
    if (pendingReplyFeedback.current) {
      invoke('record_reply_feedback', {
        input:       pendingReplyFeedback.current.input,
        aiOutput:    pendingReplyFeedback.current.aiOutput,
        accepted:    true,
        contactHint: null,
      }).catch(() => {})
      pendingReplyFeedback.current = null
    }
    try { await navigator.clipboard.writeText(streamingResult) } catch { }
    await invoke('inject_result', { text: streamingResult })
  }

  const handleFirstRunDone = async () => {
    await invoke('set_config_value', { key: 'first_run_done', value: '1' }).catch(() => { })
    setFirstRunDone(true)
  }

  const handleRestoreHistory = (entry: HistoryEntry) => {
    setCapturedText(entry.input_preview)
    setStreamingResult(entry.output)
    setSelectedMode(entry.mode as Mode)
    setShowHistory(false)
  }

  // ── Render gates ──────────────────────────────────────────────────────────
  if (hasKey === null || firstRunDone === null) return (
    <motion.div className="glass-card"
      initial={{ scale: 0.95, opacity: 0 }} animate={{ scale: 1, opacity: 1 }}
      transition={{ duration: 0.3, ease: [0.22, 1, 0.36, 1] }}
      style={{ alignItems: 'center', justifyContent: 'center', gap: '14px', minHeight: '160px' }}>
      <Sparkles className="animate-pulse" size={24} color="var(--blue)" />
      <p style={{ fontSize: '13px', color: 'var(--text-muted)', letterSpacing: '-0.01em' }}>Starting up…</p>
    </motion.div>
  )

  // Priority 4: first-run screen
  if (!firstRunDone) return <FirstRun onDone={handleFirstRunDone} />

  const canGenerate = !!capturedText && !isGenerating && (selectedMode !== 'Custom' || !!customPrompt)
  const isNonLatin = nlpContext && !['Latin', 'Unknown', ''].includes(nlpContext.language.primary_script)
  const isMixed = nlpContext?.language.is_mixed ?? false
  const isRunningLow = usage.used >= FREE_DAILY_CAP - 2
  const isAtLimit = usage.used >= usage.cap

  return (
    <motion.div className="glass-card" style={{ position: 'relative' }}
      initial={{ scale: 0.95, opacity: 0, y: 8 }} animate={{ scale: 1, opacity: 1, y: 0 }}
      transition={{ duration: 0.35, ease: [0.22, 1, 0.36, 1] }}>

      {/* ── History panel overlay ─────────────────────────────── */}
      <AnimatePresence>
        {showHistory && (
          <HistoryPanel onClose={() => setShowHistory(false)} onRestore={handleRestoreHistory} />
        )}
      </AnimatePresence>

      {/* ── Header ───────────────────────────────────────────── */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '7px' }}>
          <Sparkles size={15} color="var(--blue)" />
          <span style={{ fontWeight: 700, fontSize: '13.5px', letterSpacing: '-0.02em' }}>SnapText</span>
          <button onClick={() => setShowSettings(true)}
            style={{ background: 'none', border: 'none', padding: '0 0 0 2px', cursor: 'pointer', opacity: 0.3 }}
            title="Settings">
            <Settings size={12} color="var(--text)" />
          </button>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          {appContext && appContext !== 'other' && (
            <span style={{
              fontSize: '9px', padding: '2px 7px', borderRadius: '10px',
              background: 'rgba(255,255,255,0.06)', border: '1px solid rgba(255,255,255,0.1)',
              color: 'var(--text-dim)', fontWeight: 500
            }} title={`Active app: ${appContext}`}>
              {appContext === 'code_editor' ? '⌨ IDE' :
               appContext === 'browser' ? '🌐 Browser' :
               appContext === 'email_client' ? '✉ Email' :
               appContext === 'messaging' ? '💬 Chat' :
               appContext === 'office' ? '📄 Office' :
               appContext === 'terminal' ? '$ Terminal' :
               appContext === 'notes' ? '📝 Notes' : null}
            </span>
          )}
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
            title="Recent transforms">
            <History size={12} />
          </button>

          {/* Priority 3: usage counter — amber at 18+, red at limit */}
          <span style={{
            fontSize: '10px', fontWeight: 600,
            color: isAtLimit ? '#ef4444' : isRunningLow ? '#f59e0b' : 'var(--text-dim)',
          }}>
            {usage.used}/{usage.cap}
          </span>

          <span style={{ fontSize: '10px', color: 'var(--text-dim)' }}>
            {aiMode === 'Local' ? '⚡ Local' : aiMode === 'Byok' ? '🔒 Direct' : '🛡️ Worker'}
          </span>
        </div>
      </div>

      <AnimatePresence>
        {showSettings && (
          <SettingsModal onClose={() => setShowSettings(false)} onKeySave={() => {
            invoke<boolean>('has_api_key').then(setHasKey);
            invoke<string>('get_ai_mode').then(m => setAiMode(m.replace(/"/g, '')));
          }} />
        )}
      </AnimatePresence>

      {/* Priority 3: upgrade nudge — appears at 18+, red at limit */}
      <AnimatePresence>
        {isRunningLow && (
          <motion.div
            initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}
            exit={{ opacity: 0, height: 0 }} style={{ overflow: 'hidden', marginBottom: '8px' }}>
            <div style={{
              background: isAtLimit ? 'rgba(239,68,68,0.1)' : 'rgba(245,158,11,0.1)',
              border: `1px solid ${isAtLimit ? 'rgba(239,68,68,0.25)' : 'rgba(245,158,11,0.25)'}`,
              borderRadius: '8px', padding: '7px 12px', fontSize: '11px',
              color: isAtLimit ? '#fca5a5' : '#fcd34d',
              display: 'flex', alignItems: 'center', justifyContent: 'space-between',
            }}>
              <span>
                {isAtLimit
                  ? '⛔ Daily limit reached. Resets tomorrow.'
                  : `⚡ Running low — ${usage.cap - usage.used} transform${usage.cap - usage.used === 1 ? '' : 's'} left today.`}
              </span>
              {!isAtLimit && <span style={{ opacity: 0.6, fontSize: '10px' }}>Go Pro for unlimited</span>}
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* ── Captured text + Tone Mirror ──────────────────────── */}
      <div style={{ marginBottom: '10px' }}>
        <div className="text-preview">
          {capturedText
            ? `"${capturedText.slice(0, 120)}${capturedText.length > 120 ? '…' : ''}"`
            // Priority 2: developer-focused placeholder
            : <span style={{ opacity: 0.45 }}>Select rough text → Alt+K → get a structured prompt</span>
          }
        </div>
        {nlpContext && (
          <ToneMirror
            score={nlpContext.tone} friction={nlpContext.friction_phrases}
            wordCount={nlpContext.word_count} isRtl={nlpContext.language.is_rtl} isMixed={isMixed}
          />
        )}
      </div>

      {/* ── Suggestion bar — Priority 2: fixed 3-pill order ─── */}
      {intentResult ? (
        <SuggestionBar result={intentResult} selected={selectedMode}
          onSelect={handleSuggestionClick} isRefined={isRefined} />
      ) : (
        <div style={{ display: 'flex', gap: '6px', marginBottom: '10px' }}>
          {PRIMARY_MODES.map(m => (
            <button key={m} className={`mode-pill${selectedMode === m ? ' active' : ''}`}
              onClick={() => setSelectedMode(m)}>{m}</button>
          ))}
          <button className="mode-pill" style={{ padding: '5px 9px', opacity: 0.4, marginLeft: 'auto' }}>···</button>
        </div>
      )}

      {/* ── Custom prompt ────────────────────────────────────── */}
      <AnimatePresence>
        {selectedMode === 'Custom' && (
          <motion.div key="custom" initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }}
            style={{ overflow: 'hidden', marginBottom: '8px' }}>
            <input type="text" placeholder="E.g. 'Make it a tweet' or 'Translate to French'…"
              value={customPrompt} onChange={e => setCustomPrompt(e.target.value)} autoFocus maxLength={300} />
            <div style={{ fontSize: '10px', color: 'var(--text-dim)', textAlign: 'right', marginTop: '2px' }}>
              {customPrompt.length}/300
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* ── Output ──────────────────────────────────────────── */}
      <div className={`token-container${isGenerating ? ' blinking-cursor' : ''}`} ref={scrollRef}>
        {streamingResult
          ? streamingResult
          : isGenerating ? 'Thinking…'
            : <span style={{ color: 'var(--text-dim)' }}>
              {capturedText ? 'Ready — press Transform or Ctrl+↵' : 'Waiting for captured text…'}
            </span>
        }
        {error && <div style={{ color: '#ef4444', marginTop: '8px', fontSize: '12px' }}>⚠ {error}</div>}
        {sensitiveNotice && <div style={{ color: '#f59e0b', marginTop: '8px', fontSize: '11px', opacity: 0.8 }}>🔒 {sensitiveNotice}</div>}
      </div>

      {/* ── Actions ─────────────────────────────────────────── */}
      <div style={{ display: 'flex', gap: '7px', marginTop: '12px' }}>
        <button onClick={() => handleGenerate()} disabled={!canGenerate}
          className="mode-pill primary-action"
          style={{
            flexGrow: 1, padding: '9px',
            background: canGenerate ? 'var(--blue)' : 'rgba(255,255,255,0.05)',
            color: '#fff', fontWeight: 700, fontSize: '13px',
            boxShadow: canGenerate ? '0 0 18px var(--blue-glow)' : 'none'
          }}>
          {aiMode === 'Local' ? <Zap size={13} /> : <Send size={13} />}
          {isGenerating ? 'Generating…' : 'Transform'}
        </button>
        <button onClick={handleInsert} disabled={!streamingResult || isGenerating}
          className="mode-pill" title="Insert back (Tab)" style={{ padding: '9px 13px' }}>
          <CheckCircle2 size={15} />
        </button>
        <button onClick={async () => {
            if (!streamingResult) return
            await navigator.clipboard.writeText(streamingResult)
            setCopied(true)
            setTimeout(() => setCopied(false), 1500)
          }}
          disabled={!streamingResult} className="mode-pill" title="Copy (C)"
          style={{ padding: '9px 13px', color: copied ? '#22c55e' : undefined }}>
          {copied ? <Check size={15} /> : <Copy size={15} />}
        </button>
      </div>

      {/* ── Shortcut hints ───────────────────────────────────── */}
      <div className="shortcut-hints">
        <span>Ctrl+C×2 Pill</span>
        <span>Alt+K Overlay</span>
        <span>Alt+⇧+K Prompt</span>
        <span>Tab Insert</span>
      </div>
    </motion.div>
  )
}

export default App
