import { useState, useEffect, useRef } from 'react'
import { motion, useInView, AnimatePresence } from 'framer-motion'
import {
  Zap, Shield, Globe, Keyboard, CheckCircle2, ArrowRight,
  Key, Cpu, Server, Lock, History, MessageSquare, Settings,
  Sparkles, Eye, EyeOff, Terminal, Layers, Wrench, Clock, Gift
} from 'lucide-react'
import './App.css'

// ── Animated text demo ────────────────────────────────────────────────────

const DEMO_STEPS = [
  {
    before: 'bhai mujhe apne boss ko bolna hai ki mera kaam zyada hai aur salary kam hai, raise chahiye but politely bolna hai',
    after: 'I\'d like to schedule a brief conversation about my compensation. Over the past two years, my responsibilities have grown significantly. I believe a review would be fair at this stage.',
    pill: 'Reply',
    tag: 'Hinglish',
  },
  {
    before: 'write a leave application to my manager for 3 days, family emergency',
    after: 'Subject: Leave Application — Family Emergency\n\nDear Manager,\n\nI hope this message finds you well. I\'m writing to request leave from the office for the next 3 days due to a family emergency that requires my immediate attention. I\'ll ensure all pending tasks are handed over before I leave.\n\nThank you for your understanding.',
    pill: 'Do',
    tag: 'Instruction',
  },
  {
    before: 'hey can u plz check the server its down again and clients r angry we need fix asap',
    after: 'Hi team — the production server appears to be down and clients are impacted. Could someone investigate and share an ETA for the fix?',
    pill: 'Fix',
    tag: 'Broken English',
  },
  {
    before: 'make a prompt for generating a landing page with waitlist for my ai product',
    after: 'You are an expert web developer. Create a modern, dark-themed landing page for an AI SaaS product. Include: animated hero section, feature grid, and an email waitlist form with Formspree integration...',
    pill: 'Prompt',
    tag: 'Rough idea',
  },
  {
    before: 'yaar ye client ka mail aaya hai unko deliverables mein problem hai, kuch professional reply likh de pleej',
    after: 'Thank you for your feedback. I understand your concerns regarding the deliverables. Let me review the specific points you\'ve raised and get back to you by end of day with a revised timeline.',
    pill: 'Reply',
    tag: 'Hinglish',
  },
]

function TypeWriter({ text, className, speed = 25 }: { text: string; className: string; speed?: number }) {
  const [displayed, setDisplayed] = useState('')
  useEffect(() => {
    setDisplayed('')
    let i = 0
    const timer = setInterval(() => {
      i++
      setDisplayed(text.slice(0, i))
      if (i >= text.length) clearInterval(timer)
    }, speed)
    return () => clearInterval(timer)
  }, [text, speed])
  return (
    <span className={className}>
      {displayed}
      {displayed.length < text.length && <span style={{ opacity: 0.5, animation: 'pulse 1s ease-in-out infinite' }}>|</span>}
    </span>
  )
}

function DemoAnimation() {
  const [step, setStep] = useState(0)
  const [phase, setPhase] = useState<'before' | 'transforming' | 'after'>('before')
  useEffect(() => {
    setPhase('before')
    const t1 = setTimeout(() => setPhase('transforming'), 3000)
    const t2 = setTimeout(() => setPhase('after'), 3800)
    const t3 = setTimeout(() => setStep(s => (s + 1) % DEMO_STEPS.length), 8500)
    return () => { clearTimeout(t1); clearTimeout(t2); clearTimeout(t3) }
  }, [step])
  const current = DEMO_STEPS[step]
  return (
    <motion.div className="demo-window"
      initial={{ opacity: 0, y: 30 }} animate={{ opacity: 1, y: 0 }}
      transition={{ delay: 0.6, duration: 0.8, ease: [0.22, 1, 0.36, 1] }}>
      <div className="demo-titlebar">
        <div className="demo-dot" /><div className="demo-dot" /><div className="demo-dot" />
        <AnimatePresence mode="wait">
          <motion.span key={`tag-${step}`} initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="demo-tag">{current.tag}</motion.span>
        </AnimatePresence>
      </div>
      <div className="demo-content">
        <AnimatePresence mode="wait">
          {(phase === 'before' || phase === 'transforming') && (
            <motion.div key={`before-block-${step}`} className="demo-before-block"
              initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, height: 0, marginBottom: 0 }} transition={{ duration: 0.35 }}>
              <span className="demo-section-label">You type</span>
              <p className="demo-text before">{current.before}</p>
            </motion.div>
          )}
        </AnimatePresence>
        <AnimatePresence>
          {phase === 'transforming' && (
            <motion.div key={`shimmer-${step}`} initial={{ opacity: 0, scaleX: 0 }}
              animate={{ opacity: 1, scaleX: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.3 }}
              className="demo-shimmer" />
          )}
        </AnimatePresence>
        <AnimatePresence>
          {phase === 'after' && (
            <motion.div key={`after-block-${step}`} className="demo-after-block"
              initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.4 }}>
              <span className="demo-section-label demo-section-label--after">SnapText outputs</span>
              <p className="demo-text">
                <TypeWriter key={`after-${step}`} text={current.after} className="after" speed={16} />
              </p>
            </motion.div>
          )}
        </AnimatePresence>
        <div className="demo-pill-row">
          <div className="demo-pill">
            {['Reply', 'Do', 'Fix', 'Prompt'].map(label => (
              <span key={label} className={`demo-pill-btn${current.pill === label ? ' active' : ''}`}>{label}</span>
            ))}
          </div>
          <span className="demo-label">
            {phase === 'before' ? 'Ctrl+C, Ctrl+C to transform' : phase === 'after' ? 'Injected into your text field' : ''}
          </span>
        </div>
      </div>
    </motion.div>
  )
}

// ── Fade-in wrapper ───────────────────────────────────────────────────────

function FadeIn({ children, delay = 0, className = '' }: {
  children: React.ReactNode; delay?: number; className?: string
}) {
  const ref = useRef(null)
  const isInView = useInView(ref, { once: true, margin: '-60px' })
  return (
    <motion.div ref={ref} className={className}
      initial={{ opacity: 0, y: 24 }}
      animate={isInView ? { opacity: 1, y: 0 } : {}}
      transition={{ duration: 0.6, delay, ease: [0.22, 1, 0.36, 1] }}>
      {children}
    </motion.div>
  )
}

// ── Pill visual demo ──────────────────────────────────────────────────────

function PillVisual() {
  const [active, setActive] = useState('Reply')
  return (
    <div className="pill-visual">
      <div className="pill-visual-window">
        <div className="pill-visual-text">
          yaar mujhe client ko politely batana hai ki deadline extend hoga
          <span className="pill-visual-cursor">|</span>
        </div>
        <div className="pill-visual-floating">
          {['Reply', 'Do', 'Fix', 'Prompt'].map(m => (
            <button key={m} onClick={() => setActive(m)}
              className={`pill-visual-btn${active === m ? ' active' : ''}`}>{m}</button>
          ))}
        </div>
      </div>
      <p className="pill-visual-caption">
        The pill floats right below your text — click any mode and the result is pasted back instantly. Click anywhere else to dismiss.
      </p>
    </div>
  )
}

// ── Overlay visual demo ───────────────────────────────────────────────────

function OverlayVisual() {
  const [mode, setMode] = useState<'Cloud' | 'BYOK' | 'Local'>('Cloud')
  return (
    <div className="overlay-visual">
      <div className="overlay-mock">
        <div className="overlay-header">
          <div className="overlay-header-left">
            <Sparkles size={14} color="#6366f1" />
            <span>SnapText</span>
            <Settings size={11} style={{ opacity: 0.3 }} />
          </div>
          <div className="overlay-header-right">
            <span className="overlay-badge">MIXED</span>
            <History size={12} style={{ opacity: 0.4 }} />
            <span className="overlay-usage">3/20</span>
          </div>
        </div>
        <div className="overlay-preview">"bhai mujhe salary raise ke baare mein boss se baat karni hai..."</div>
        <div className="overlay-tone">
          <span className="overlay-tone-dot" />
          Hinglish / mixed language detected
        </div>
        <div className="overlay-modes">
          {['Reply', 'Prompt', 'Correct', 'Translate'].map(m => (
            <span key={m} className={`overlay-mode${m === 'Reply' ? ' active' : ''}`}>{m}</span>
          ))}
          <span className="overlay-mode" style={{ opacity: 0.4 }}>···</span>
        </div>
        <div className="overlay-output">
          I'd like to schedule a brief conversation about my compensation...
          <span className="overlay-cursor">▋</span>
        </div>
        <div className="overlay-actions">
          <span className="overlay-btn primary">Transform</span>
          <span className="overlay-btn">Insert</span>
          <span className="overlay-btn">Copy</span>
        </div>

        {/* Settings panel */}
        <div className="overlay-settings">
          <span className="overlay-settings-label">INFERENCE ENGINE</span>
          <div className="overlay-engine-toggle">
            {(['Cloud', 'BYOK', 'Local'] as const).map(m => (
              <button key={m} onClick={() => setMode(m)}
                className={`overlay-engine-btn${mode === m ? ' active' : ''}`}>{m}</button>
            ))}
          </div>
          <AnimatePresence mode="wait">
            {mode === 'BYOK' && (
              <motion.div key="byok" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }} className="overlay-engine-detail">
                <Key size={13} /> Paste your Gemini API key — your key, your quota, zero middleman.
              </motion.div>
            )}
            {mode === 'Local' && (
              <motion.div key="local" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }} className="overlay-engine-detail">
                <Cpu size={13} /> Runs on your GPU via Ollama — completely offline, 100% private.
              </motion.div>
            )}
            {mode === 'Cloud' && (
              <motion.div key="cloud" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }} className="overlay-engine-detail">
                <Server size={13} /> 20 free transforms/day — no API key needed.
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      </div>
    </div>
  )
}

// ── Main App ──────────────────────────────────────────────────────────────

// ── Countdown to launch ────────────────────────────────────────────────────

// Fixed launch deadline: April 23, 2026 11:00 AM — 24h window starting Apr 22 11 AM
const LAUNCH_DEADLINE = new Date('2026-04-23T11:00:00').getTime()

function getLaunchDeadline(): number {
  // Clear any stale per-visitor deadline so everyone sees the same timer
  if (typeof window !== 'undefined') localStorage.removeItem('snaptext_deadline')
  return LAUNCH_DEADLINE
}

function Countdown() {
  const [deadline] = useState(getLaunchDeadline)
  const [time, setTime] = useState({ d: 0, h: 0, m: 0, s: 0 })

  useEffect(() => {
    const tick = () => {
      const diff = deadline - Date.now()
      if (diff <= 0) { setTime({ d: 0, h: 0, m: 0, s: 0 }); return }
      const d = Math.floor(diff / (1000 * 60 * 60 * 24))
      const h = Math.floor((diff % (1000 * 60 * 60 * 24)) / (1000 * 60 * 60))
      const m = Math.floor((diff % (1000 * 60 * 60)) / (1000 * 60))
      const s = Math.floor((diff % (1000 * 60)) / 1000)
      setTime({ d, h, m, s })
    }
    tick()
    const interval = setInterval(tick, 1000)
    return () => clearInterval(interval)
  }, [deadline])

  const pad = (n: number) => n.toString().padStart(2, '0')
  return (
    <div className="countdown">
      <div className="countdown-block">
        <span className="countdown-num">{pad(time.d)}</span>
        <span className="countdown-label">days</span>
      </div>
      <span className="countdown-sep">:</span>
      <div className="countdown-block">
        <span className="countdown-num">{pad(time.h)}</span>
        <span className="countdown-label">hours</span>
      </div>
      <span className="countdown-sep">:</span>
      <div className="countdown-block">
        <span className="countdown-num">{pad(time.m)}</span>
        <span className="countdown-label">mins</span>
      </div>
      <span className="countdown-sep">:</span>
      <div className="countdown-block">
        <span className="countdown-num">{pad(time.s)}</span>
        <span className="countdown-label">secs</span>
      </div>
    </div>
  )
}

export default function App() {
  const [email, setEmail] = useState('')
  const [submitted, setSubmitted] = useState(false)
  const [submitting, setSubmitting] = useState(false)

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!email || submitting) return
    setSubmitting(true)
    try {
      await fetch('https://formspree.io/f/xvzvlnwj', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email }),
      })
      setSubmitted(true)
    } catch {
      setSubmitted(true)
    }
    setSubmitting(false)
  }

  return (
    <div className="page">
      <div className="orb orb-1" />
      <div className="orb orb-2" />
      <div className="orb orb-3" />

      {/* ── Nav ───────────────────────────────────────── */}
      <nav className="nav">
        <div className="nav-inner">
          <div className="logo">
            <img src="/icon.png" alt="SnapText" />
            SnapText
          </div>
          <div className="nav-links">
            <a href="#how-it-works">How it works</a>
            <a href="#modes">Modes</a>
            <a href="#security">Security</a>
            <a href="#setup">Setup</a>
            <a href="#android">Android</a>
          </div>
          <a href="#waitlist" className="nav-cta">Join Waitlist</a>
        </div>
      </nav>

      {/* ══════════════════════════════════════════════════
          1. HERO
      ══════════════════════════════════════════════════ */}
      <section className="hero">
        <motion.div className="hero-badge"
          initial={{ opacity: 0, scale: 0.9 }} animate={{ opacity: 1, scale: 1 }} transition={{ delay: 0.2 }}>
          <span className="dot" />
          Now on Windows & Android
        </motion.div>
        <motion.h1 initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.3, duration: 0.7, ease: [0.22, 1, 0.36, 1] }}>
          Your text,{' '}<span className="gradient">instantly better</span>
        </motion.h1>
        <motion.p className="hero-sub"
          initial={{ opacity: 0, y: 16 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: 0.45, duration: 0.7 }}>
          Select any text. Double-tap copy. SnapText rewrites, fixes grammar,
          composes replies from Hinglish instructions — and pastes it right back.
        </motion.p>
        <DemoAnimation />
      </section>

      {/* ══════════════════════════════════════════════════
          2. HOW THE PILL WORKS
      ══════════════════════════════════════════════════ */}
      <section className="section-dark" id="how-it-works">
        <div className="container">
          <FadeIn>
            <div style={{ textAlign: 'center', marginBottom: '56px' }}>
              <p className="section-label">How it works</p>
              <h2 className="section-title">Three keystrokes. Zero friction.</h2>
            </div>
          </FadeIn>
          <div className="steps-grid">
            {[
              { num: '1', title: 'Select your text', desc: 'Highlight any text in any app — emails, docs, chat, code editors. SnapText works everywhere.' },
              { num: '2', title: 'Double-copy', desc: 'Press Ctrl+C twice quickly. The SnapText pill floats right below your text with mode options.', keys: ['Ctrl', 'C', 'C'] },
              { num: '3', title: 'Pick & inject', desc: 'Click Reply, Do, Fix, or Prompt. AI transforms your text and pastes the result back instantly.' },
            ].map((step, i) => (
              <FadeIn key={i} delay={i * 0.12}>
                <div className="step-card">
                  <div className="step-number">{step.num}</div>
                  <h3>{step.title}</h3>
                  <p>{step.desc}</p>
                  {step.keys && <div className="step-keys">{step.keys.map(k => <span key={k} className="key">{k}</span>)}</div>}
                </div>
              </FadeIn>
            ))}
          </div>

          {/* Pill visual */}
          <FadeIn delay={0.2}>
            <div style={{ marginTop: '64px' }}>
              <h3 className="subsection-title">The floating pill</h3>
              <p className="subsection-desc">Appears right below your cursor after double-copy. Click anywhere else to dismiss.</p>
              <PillVisual />
            </div>
          </FadeIn>
        </div>
      </section>

      {/* ══════════════════════════════════════════════════
          3. ALL MODES
      ══════════════════════════════════════════════════ */}
      <section className="section-alt" id="modes">
        <div className="container">
          <FadeIn>
            <div style={{ textAlign: 'center', marginBottom: '56px' }}>
              <p className="section-label">Modes</p>
              <h2 className="section-title">One tool, many superpowers.</h2>
            </div>
          </FadeIn>
          <div className="modes-grid">
            {[
              { icon: <MessageSquare size={20} />, color: 'indigo', name: 'Reply', desc: 'Describe what you want to say in any language — SnapText composes the actual message. "Boss ko politely bolna hai" → professional salary discussion.' },
              { icon: <CheckCircle2 size={20} />, color: 'emerald', name: 'Fix', desc: 'Rewrites broken English, Hinglish, or any language into clear, fluent, professional English. Grammar, spelling, structure — all fixed.' },
              { icon: <Shield size={20} />, color: 'purple', name: 'Pro', desc: 'Same text, but rewritten with a confident, professional tone. Perfect for emails to bosses, clients, or stakeholders.' },
              { icon: <Sparkles size={20} />, color: 'amber', name: 'Prompt', desc: 'Turns rough ideas into structured AI prompts with Role, Context, Task, and Constraints. Even turns error logs into debug prompts.' },
              { icon: <Globe size={20} />, color: 'emerald', name: 'Translate', desc: 'Detects your language and translates. Handles Hinglish, mixed scripts, RTL languages, and cultural nuances.' },
              { icon: <Layers size={20} />, color: 'indigo', name: 'Email', desc: 'Converts rough notes into a properly structured email with subject line, greeting, body, and sign-off.' },
              { icon: <Zap size={20} />, color: 'amber', name: 'Casual / Strategist / Knowledge', desc: 'Casual rewrites for friends. Brand strategist analysis for marketers. Expert explanations for learners.' },
              { icon: <Wrench size={20} />, color: 'purple', name: 'Custom', desc: 'Write your own instruction: "Make it a tweet", "Translate to French", "Summarize in 3 bullets". Anything goes.' },
            ].map((mode, i) => (
              <FadeIn key={i} delay={i * 0.06}>
                <div className="mode-card">
                  <div className={`feature-icon ${mode.color}`}>{mode.icon}</div>
                  <h3>{mode.name}</h3>
                  <p>{mode.desc}</p>
                </div>
              </FadeIn>
            ))}
          </div>
        </div>
      </section>

      {/* ══════════════════════════════════════════════════
          4. FULL OVERLAY + BYOK + ENGINE OPTIONS
      ══════════════════════════════════════════════════ */}
      <section className="section-dark" id="setup">
        <div className="container">
          <FadeIn>
            <div style={{ textAlign: 'center', marginBottom: '56px' }}>
              <p className="section-label">The Full Overlay</p>
              <h2 className="section-title">Press Alt+K for the complete experience.</h2>
              <p className="section-sub">Preview results before inserting. Access all modes, history, tone analysis, and AI settings — all in one floating overlay.</p>
            </div>
          </FadeIn>

          <FadeIn delay={0.15}>
            <OverlayVisual />
          </FadeIn>

          {/* Engine options */}
          <div className="engine-grid" style={{ marginTop: '72px' }}>
            <FadeIn delay={0.1}>
              <div className="engine-card">
                <div className="feature-icon indigo"><Server size={20} /></div>
                <h3>Cloud (Default)</h3>
                <p>20 free transforms/day. No API key, no setup. Just install and go.</p>
                <ul className="engine-pros">
                  <li><CheckCircle2 size={12} /> Zero configuration</li>
                  <li><CheckCircle2 size={12} /> Works immediately</li>
                  <li><CheckCircle2 size={12} /> Powered by Gemini 2.5 Flash</li>
                </ul>
              </div>
            </FadeIn>
            <FadeIn delay={0.2}>
              <div className="engine-card highlight">
                <div className="feature-icon purple"><Key size={20} /></div>
                <h3>BYOK — Bring Your Own Key</h3>
                <p>Paste your Gemini API key. Unlimited transforms. Your key, your quota, zero middleman.</p>
                <ul className="engine-pros">
                  <li><CheckCircle2 size={12} /> Unlimited usage</li>
                  <li><CheckCircle2 size={12} /> Direct API — no proxy</li>
                  <li><CheckCircle2 size={12} /> Key stored locally, encrypted</li>
                </ul>
                <div className="engine-setup">
                  <span className="engine-setup-label">Setup:</span>
                  <span>Get a free key from <strong>aistudio.google.com</strong> → paste in Settings → done.</span>
                </div>
              </div>
            </FadeIn>
            <FadeIn delay={0.3}>
              <div className="engine-card">
                <div className="feature-icon emerald"><Cpu size={20} /></div>
                <h3>Local — Ollama</h3>
                <p>Run AI on your own GPU. Completely offline. Zero data leaves your machine.</p>
                <ul className="engine-pros">
                  <li><CheckCircle2 size={12} /> 100% offline & private</li>
                  <li><CheckCircle2 size={12} /> No internet required</li>
                  <li><CheckCircle2 size={12} /> Supports phi3, gemma2, llama3</li>
                </ul>
                <div className="engine-setup">
                  <span className="engine-setup-label">Setup:</span>
                  <div className="engine-cmd">
                    <Terminal size={12} />
                    <code>ollama pull phi3</code>
                  </div>
                  <span>Install <strong>ollama.com</strong> → pull a model → select Local in Settings.</span>
                </div>
              </div>
            </FadeIn>
          </div>
        </div>
      </section>

      {/* ══════════════════════════════════════════════════
          5. SECURITY
      ══════════════════════════════════════════════════ */}
      <section className="section-alt" id="security">
        <div className="container">
          <FadeIn>
            <div style={{ textAlign: 'center', marginBottom: '56px' }}>
              <p className="section-label">Security & Privacy</p>
              <h2 className="section-title">Your text stays yours.</h2>
            </div>
          </FadeIn>
          <div className="security-grid">
            {[
              { icon: <EyeOff size={20} />, title: 'No data stored', desc: 'Text is processed and immediately discarded. We never log, save, or train on your content. History stays on your device.' },
              { icon: <Lock size={20} />, title: 'No browser extension', desc: 'SnapText is a native desktop app — it doesn\'t inject into your browser, read your DOM, or access your cookies.' },
              { icon: <Shield size={20} />, title: 'API keys encrypted locally', desc: 'BYOK keys are stored in your local SQLite database with encryption. They never leave your machine.' },
              { icon: <Cpu size={20} />, title: 'Fully offline mode', desc: 'Use Ollama and your text never touches the internet. Zero network calls. Pure local inference.' },
              { icon: <Eye size={20} />, title: 'Clipboard-only access', desc: 'SnapText only reads clipboard on double-copy gesture. It does not keylog, screen-capture, or monitor your typing.' },
              { icon: <Server size={20} />, title: 'Cloudflare Worker proxy', desc: 'Cloud mode routes through a Cloudflare Worker — no persistent server. Request processed, response returned, nothing stored.' },
            ].map((item, i) => (
              <FadeIn key={i} delay={i * 0.08}>
                <div className="security-card">
                  <div className="feature-icon emerald">{item.icon}</div>
                  <h3>{item.title}</h3>
                  <p>{item.desc}</p>
                </div>
              </FadeIn>
            ))}
          </div>
        </div>
      </section>

      {/* ══════════════════════════════════════════════════
          6. EASE OF USE — QUICK START
      ══════════════════════════════════════════════════ */}
      <section className="section-dark">
        <div className="container">
          <FadeIn>
            <div style={{ textAlign: 'center', marginBottom: '56px' }}>
              <p className="section-label">Getting Started</p>
              <h2 className="section-title">Install → Type → Transform. That's it.</h2>
            </div>
          </FadeIn>
          <div className="quickstart-grid">
            {[
              { step: '01', title: 'Install the .msi', desc: 'Double-click the installer. No Node.js, no Python, no dependencies. Under 6 MB.' },
              { step: '02', title: 'It lives in your tray', desc: 'SnapText runs silently in the system tray. No window to manage. It\'s always ready.' },
              { step: '03', title: 'Select text anywhere', desc: 'Any app, any field — Chrome, Slack, VS Code, Notion, WhatsApp Desktop, even Notepad.' },
              { step: '04', title: 'Ctrl+C, Ctrl+C', desc: 'Double-copy triggers the floating pill. Or press Alt+K for the full overlay with preview.' },
              { step: '05', title: 'Pick your mode', desc: 'Reply, Do, Fix, Prompt, Translate, Email, Custom — pick what fits your need.' },
              { step: '06', title: 'Result injected', desc: 'The AI output is pasted right back into your active text field. No copy-paste needed.' },
            ].map((item, i) => (
              <FadeIn key={i} delay={i * 0.06}>
                <div className="quickstart-card">
                  <span className="quickstart-num">{item.step}</span>
                  <h3>{item.title}</h3>
                  <p>{item.desc}</p>
                </div>
              </FadeIn>
            ))}
          </div>
        </div>
      </section>

      {/* ══════════════════════════════════════════════════
          7. ALL FEATURES OVERVIEW
      ══════════════════════════════════════════════════ */}
      <section className="section-alt">
        <div className="container">
          <FadeIn>
            <div style={{ textAlign: 'center', marginBottom: '56px' }}>
              <p className="section-label">Everything Included</p>
              <h2 className="section-title">Built for speed, not settings.</h2>
            </div>
          </FadeIn>
          <div className="features-grid">
            {[
              { icon: <Zap size={20} />, color: 'indigo', title: 'Under 2 seconds', desc: 'From keypress to injected result. Silent hotkeys (Alt+Shift+K/L) skip the overlay entirely.' },
              { icon: <Keyboard size={20} />, color: 'purple', title: 'Works in any app', desc: 'Chrome, Slack, VS Code, Notion, WhatsApp — anywhere you can select text. No integrations to configure.' },
              { icon: <Globe size={20} />, color: 'amber', title: 'Hinglish → English', desc: 'Write in Hinglish, Hindi, Spanish, or mixed scripts. SnapText detects language, script, and tone automatically.' },
              { icon: <History size={20} />, color: 'emerald', title: 'Transform history', desc: 'Every transform saved locally. Restore, reuse, or review past outputs. All stored on-device.' },
              { icon: <MessageSquare size={20} />, color: 'indigo', title: 'Tone analysis', desc: 'Real-time tone mirror: warns if your text sounds passive-aggressive, detects friction phrases, suggests de-escalation.' },
              { icon: <Sparkles size={20} />, color: 'purple', title: 'Voice learning', desc: 'SnapText learns your writing style — your openers, closers, vocabulary. Outputs sound like you, not a generic AI.' },
              { icon: <Layers size={20} />, color: 'amber', title: 'Smart NLP routing', desc: 'Analyzes your text locally before sending to AI: intent detection, language identification, formality scoring — all on-device.' },
              { icon: <History size={20} />, color: 'indigo', title: 'Gets smarter every use', desc: 'Local RAG memory: finds your past similar transforms and injects them as context. The more you use it, the more outputs match your exact style and format — 100% on-device.' },
              { icon: <Settings size={20} />, color: 'emerald', title: 'Dev-friendly', desc: 'Paste error logs → get structured debug prompts. Paste SQL → get optimization advice. Paste code → get reviews.' },
            ].map((feat, i) => (
              <FadeIn key={i} delay={i * 0.06}>
                <div className="feature-card">
                  <div className={`feature-icon ${feat.color}`}>{feat.icon}</div>
                  <h3>{feat.title}</h3>
                  <p>{feat.desc}</p>
                </div>
              </FadeIn>
            ))}
          </div>
        </div>
      </section>

      {/* ══════════════════════════════════════════════════
          8. ANDROID
      ══════════════════════════════════════════════════ */}
      <section className="section-dark" id="android">
        <div className="container">
          <FadeIn>
            <div style={{ textAlign: 'center', marginBottom: '56px' }}>
              <p className="section-label">Now on Android</p>
              <h2 className="section-title">Skip ChatGPT on your phone.</h2>
              <p className="section-subtitle" style={{ maxWidth: '560px', margin: '16px auto 0' }}>
                Select any text in WhatsApp, Instagram, Chrome — tap <strong>SnapText</strong> from the menu. Reply generates instantly.
              </p>
            </div>
          </FadeIn>

          {/* Phone mockup */}
          <FadeIn delay={0.15}>
            <div className="phone-demo">
              <div className="phone-frame">
                <div className="phone-notch" />
                <div className="phone-screen">
                  {/* Chat bubbles */}
                  <div className="phone-chat">
                    <div className="chat-bubble incoming">
                      bhai kal tak report chahiye, client bohot irritate ho raha hai. jaldi kar de please
                    </div>
                    <div className="chat-bubble-time">11:42 AM</div>
                    <div className="chat-selection-menu">
                      <span className="selection-option">Cut</span>
                      <span className="selection-option">Copy</span>
                      <span className="selection-option">Paste</span>
                      <span className="selection-option active">SnapText</span>
                    </div>
                    <div className="snaptext-sheet">
                      <div className="sheet-header">
                        <span className="sheet-title">&#10024; SnapText</span>
                        <span className="sheet-close">&#10005;</span>
                      </div>
                      <div className="sheet-result">
                        Haan bhai, samajh gaya. Kal subah tak bhej dunga,
                        client ko bol de thoda patience rakhe. Kaam ho jayega.
                      </div>
                      <div className="sheet-actions-row">
                        <span className="sheet-chip">&#9889; Do</span>
                        <span className="sheet-chip">&#9989; Fix English</span>
                        <span className="sheet-chip">&#10024; AI Prompt</span>
                      </div>
                      <div className="sheet-buttons">
                        <span className="sheet-btn">Copy</span>
                        <span className="sheet-btn primary">Insert</span>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </FadeIn>

          {/* How it works on Android */}
          <div className="android-steps">
            {[
              { step: '01', title: 'Select text in any app', desc: 'Long-press any message in WhatsApp, email in Gmail, or text in Chrome.' },
              { step: '02', title: 'Tap "SnapText"', desc: 'One option in the menu. Reply starts generating immediately — no mode selection.' },
              { step: '03', title: 'Copy or Insert', desc: 'Tap Insert to replace the text, or Copy to paste it yourself. Done in 3 seconds.' },
            ].map((item, i) => (
              <FadeIn key={i} delay={i * 0.1}>
                <div className="android-step-card">
                  <span className="quickstart-num">{item.step}</span>
                  <h3>{item.title}</h3>
                  <p>{item.desc}</p>
                </div>
              </FadeIn>
            ))}
          </div>

          {/* The 4 actions */}
          <FadeIn delay={0.2}>
            <div style={{ textAlign: 'center', marginTop: '64px', marginBottom: '32px' }}>
              <p className="section-label">Four actions, one tap each</p>
              <h3 style={{ fontSize: '22px', fontWeight: 700, color: 'var(--text)', marginTop: '8px' }}>
                Reply by default. Switch instantly.
              </h3>
            </div>
          </FadeIn>

          <div className="android-actions-grid">
            {[
              {
                icon: '\uD83D\uDCAC',
                title: 'Reply',
                badge: 'Default',
                desc: 'Auto-runs the moment you tap SnapText. Composes a reply in the same language and tone as the input — Hinglish stays Hinglish.',
              },
              {
                icon: '\u26A1',
                title: 'Do',
                badge: 'New',
                desc: 'Selected text is an instruction? SnapText reads it and DOES it. "Write a leave message to my boss" → leave message appears. No prompts to write.',
              },
              {
                icon: '\u2705',
                title: 'Fix English',
                badge: '',
                desc: 'Broken English, Hinglish, typos, awkward phrasing — rewritten into clean, professional English while preserving your meaning.',
              },
              {
                icon: '\u2728',
                title: 'AI Prompt',
                badge: '',
                desc: 'Turns rough ideas into structured AI prompts with Role, Context, Task, Constraints — paste-ready for ChatGPT, Claude, or Gemini.',
              },
            ].map((action, i) => (
              <FadeIn key={i} delay={i * 0.08}>
                <div className="android-action-card">
                  <div className="android-action-header">
                    <span className="android-action-icon">{action.icon}</span>
                    <h4>{action.title}</h4>
                    {action.badge && <span className={`android-action-badge ${action.badge.toLowerCase()}`}>{action.badge}</span>}
                  </div>
                  <p>{action.desc}</p>
                </div>
              </FadeIn>
            ))}
          </div>

          <div style={{ height: '48px' }} />

          {/* Key differentiators */}
          <FadeIn delay={0.2}>
            <div className="android-features">
              {[
                { icon: <Globe size={18} />, text: 'Replies in your language — Hinglish in, Hinglish out' },
                { icon: <Zap size={18} />, text: 'No keyboard to install. No scary permissions.' },
                { icon: <MessageSquare size={18} />, text: 'Works alongside Gboard. Not instead of it.' },
                { icon: <Shield size={18} />, text: 'Free — 20 transforms/day. BYOK for unlimited.' },
              ].map((item, i) => (
                <div key={i} className="android-feature-item">
                  <div className="feature-icon indigo">{item.icon}</div>
                  <span>{item.text}</span>
                </div>
              ))}
            </div>
          </FadeIn>
        </div>
      </section>

      {/* ══════════════════════════════════════════════════
          9. WAITLIST CTA
      ══════════════════════════════════════════════════ */}
      <section className="waitlist" id="waitlist">
        <div className="container">
          <FadeIn>
            <div className="waitlist-box urgency">
              {/* Urgency badge */}
              <div className="urgency-badge">
                <Clock size={12} />
                <span>Early access closes in</span>
              </div>

              {/* Countdown */}
              <Countdown />

              <h2>First 100 users get it free. Forever.</h2>
              <p>
                We're opening early access for the next <strong>24 hours only</strong>.
                The first 100 people on the waitlist get <strong>unlimited transforms — lifetime free</strong>.
                After that, it's 20/day or BYOK.
              </p>

              {/* Perks row */}
              <div className="urgency-perks">
                <div className="urgency-perk">
                  <Gift size={14} />
                  <span>Lifetime unlimited</span>
                </div>
                <div className="urgency-perk">
                  <Zap size={14} />
                  <span>Both Windows + Android</span>
                </div>
                <div className="urgency-perk">
                  <Sparkles size={14} />
                  <span>First in line for new features</span>
                </div>
              </div>

              {!submitted ? (
                <form className="waitlist-form" onSubmit={handleSubmit}>
                  <input type="email" placeholder="you@email.com" value={email}
                    onChange={e => setEmail(e.target.value)} required />
                  <button type="submit" disabled={submitting}>
                    {submitting ? 'Joining...' : <>Claim my spot <ArrowRight size={14} style={{ marginLeft: 4 }} /></>}
                  </button>
                </form>
              ) : (
                <motion.div className="waitlist-success"
                  initial={{ opacity: 0, scale: 0.95 }} animate={{ opacity: 1, scale: 1 }}>
                  <CheckCircle2 size={18} />
                  You're in. Check your email in the next 24 hours.
                </motion.div>
              )}

              <p className="urgency-fineprint">
                No credit card. No spam. We'll email you the download links when access opens.
              </p>
            </div>
          </FadeIn>
        </div>
      </section>

      {/* ── Footer ────────────────────────────────────── */}
      <footer className="footer">
        <div className="container">
          SnapText {new Date().getFullYear()} — Built for people who type a lot.
        </div>
      </footer>
    </div>
  )
}
