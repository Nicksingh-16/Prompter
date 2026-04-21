# SnapText — Roadmap

## Vision

SnapText replaces the "copy → ChatGPT → paste" workflow for Indian professionals. One tap to reply to clients, fix your English, or compose messages — right where you type.

**Target user:** Working professionals in India who reply to clients/colleagues 15-20 times daily and currently use ChatGPT as a crutch.

**Core positioning:** Not an "AI keyboard." A professional communication shortcut.

---

## Phase 1: Ship What Works (Current Sprint)

**Goal:** Strip to 4 core modes, make PROCESS_TEXT flow bulletproof, record first LinkedIn demo.

### Strip Mobile to Core
- [ ] Remove from keyboard modes: Translate, Expand, Summarize (keep as hidden/secondary)
- [ ] Keep only: **Reply, Correct, Professional, Email** as primary visible modes
- [ ] Remove tools row (clipboard, templates) from default keyboard view
- [ ] Move clipboard/templates to settings or long-press AI button
- [ ] Kill analytics dashboard from settings (premature — no users yet)
- [ ] Simplify settings: just AI provider + enable keyboard

### Fix Core UX Bugs
- [ ] Test backspace long-press repeat thoroughly (400ms initial, 50ms repeat)
- [ ] Test voice input with proper RECORD_AUDIO permission request flow
- [ ] Verify Tone Guard doesn't slow down typing (600ms debounce)
- [ ] Test PROCESS_TEXT flow in WhatsApp, Instagram, Chrome, Gmail
- [ ] Ensure AI bar doesn't overlap with mode chips when scrolling
- [ ] Test on low-end devices (2GB RAM, Android 8)

### Language Quality
- [ ] Test Reply mode with Hinglish input — must reply in Hinglish
- [ ] Test Reply mode with Hindi (Devanagari) — must reply in Hindi
- [ ] Test Reply with Rajasthani/regional dialect markers
- [ ] Test Correct mode — must convert to clean English
- [ ] Test Professional mode — must keep input language
- [ ] Verify prompts don't produce "Here is your reply:" preamble

### First Demo
- [ ] Record 3 screen recordings of real workflows:
  1. WhatsApp client message → Reply in Hinglish
  2. Broken English email → Correct to professional English
  3. Quick notes → Professional email
- [ ] Post first LinkedIn video with workflow comparison (ChatGPT vs SnapText)

---

## Phase 2: Validate Demand (Week 2-3)

**Goal:** Get 100 real users, measure retention.

### Distribution
- [ ] LinkedIn content: 2-3 posts/week showing real before/after workflows
- [ ] Identify 5-10 freelancer/professional communities to seed
- [ ] Direct outreach to 20 people who match target profile
- [ ] Play Store listing optimized for "WhatsApp reply assistant" / "professional English helper"

### Metrics to Track
- [ ] Daily active users (via worker endpoint logs)
- [ ] Transforms per user per day
- [ ] Which mode used most (Reply? Correct?)
- [ ] Retention: day 1, day 3, day 7 return rate
- [ ] Drop-off point: where do users stop?

### Quick Wins Based on Feedback
- [ ] If Reply mode is 80%+ usage → make it the default, skip mode selection
- [ ] If users hit 20/day cap and complain → demand is real, monetization path exists
- [ ] If users uninstall after keyboard permission screen → prioritize PROCESS_TEXT flow over IME

---

## Phase 3: Product-Market Fit (Week 4-8)

**Goal:** Find the one thing users love, double down.

### Likely Scenarios

**Scenario A: PROCESS_TEXT wins over keyboard**
People don't want to switch keyboards but love the "select text → Reply with SnapText" flow.
- [ ] Make PROCESS_TEXT the primary product
- [ ] Add notification-based flow: copy text → notification appears → tap → reply ready
- [ ] Consider killing the keyboard entirely — reduce permission friction
- [ ] Rebrand from "keyboard" to "reply assistant"

**Scenario B: Keyboard sticks with power users**
Freelancers/sales who reply 30+ times daily prefer the keyboard.
- [ ] Add voice profile learning (port from desktop)
- [ ] Add client/contact context memory
- [ ] Optimize keyboard for speed: one-tap Reply without mode selection
- [ ] Add "favorite modes" — user picks their top 3

**Scenario C: Correct mode is the killer feature**
English anxiety is the real pain point, not reply generation.
- [ ] Build dedicated "English Coach" mode
- [ ] Show what was wrong + why (grammar explanation)
- [ ] Track improvement over time
- [ ] This becomes the education play

### Voice Profile (Port from Desktop)
- [ ] SQLite database on mobile for voice profile storage
- [ ] Track opener/closer patterns
- [ ] Track vocabulary fingerprint
- [ ] Feed into Reply prompt so output sounds like the user
- [ ] Track per-contact tone (formal with boss, casual with friend)

---

## Phase 4: Monetization (After 1K+ DAU)

**Goal:** Sustainable revenue without killing growth.

### Pricing Model
```
FREE:     15-20 transforms/day (enough to get hooked)
PRO:      ₹29/mo or ₹249/year
```

### What PRO Unlocks
- Unlimited transforms
- Voice profile learning (AI sounds like you)
- Priority API (faster responses)
- No ads (if ads are added to free tier)

### When to Show Paywall
- NOT on first session
- Day 4-5, after user has done 50+ transforms
- Show: "You've improved X messages this week" + upgrade prompt
- Soft cap: after 15/day, show "Upgrade for unlimited" but allow 5 more

### Alternative Revenue (If Consumer Pricing Fails)
- **B2B: SnapText for Teams** — ₹199/seat/mo for sales and support teams
- **API licensing** — Let other apps use the Hinglish-aware rewrite engine
- **Ad-supported free tier** — Small banner in result panel (₹5-15 CPM)

---

## Phase 5: Platform Expansion

### Desktop Polish
- [ ] Mac support (Tauri already cross-platform)
- [ ] Linux support
- [ ] Browser extension (Chrome/Edge) — intercept text fields directly
- [ ] VS Code extension — code comment/docs generation

### iOS
- [ ] iOS keyboard extension (Swift)
- [ ] Share sheet integration (like PROCESS_TEXT)
- [ ] Port NLP engine to Swift

### Integrations
- [ ] WhatsApp Business API — direct reply suggestions in business chats
- [ ] Slack bot — team communication assistant
- [ ] Gmail add-on — compose/reply from sidebar

---

## Technical Debt to Address

### Mobile
- [ ] Move from SharedPreferences JSON to Room/SQLite for clipboard and templates
- [ ] Add proper error handling for network failures (retry logic, offline queue)
- [ ] Add ProGuard rules for OkHttp/coroutines
- [ ] Implement proper keyboard height calculation (not fixed dp)
- [ ] Add swipe-to-type (major UX gap vs Gboard)
- [ ] Accessibility: TalkBack support, content descriptions

### Desktop
- [ ] Replace XOR key storage with OS keychain (Windows Credential Manager)
- [ ] Add auto-update mechanism
- [ ] Reduce binary size (currently bundles full WebView)
- [ ] Add crash reporting

### Infrastructure
- [ ] Move from Cloudflare KV to D1 or Supabase for better analytics
- [ ] Add basic telemetry (anonymous usage stats, crash reports)
- [ ] Set up CI/CD for Play Store releases
- [ ] API key rotation automation

---

## What We're NOT Building (Intentional Cuts)

| Feature | Why Not |
|---------|---------|
| Swipe typing | Can't compete with Gboard's swipe engine. Not our differentiator. |
| Autocorrect/prediction | Same reason. Google has 10 years of data. |
| Theme customization | Vanity feature. Doesn't drive retention. |
| Sticker/GIF keyboard | Off-mission. We're a productivity tool. |
| Social features | Not a social app. |
| Gamification/streaks | Feels forced. The product should be useful, not addictive. |
| Multi-language keyboard layouts | Gboard already does this perfectly. |

---

## Success Metrics

| Milestone | Target | When |
|-----------|--------|------|
| First 100 users | Organic from LinkedIn + communities | Week 2-3 |
| 50% D7 retention | Half of installers still using after 1 week | Week 3-4 |
| 1K DAU | Daily active users | Month 2-3 |
| First paying user | Someone upgrades to Pro | Month 2-3 |
| 10K DAU | Enough for meaningful ad revenue or B2B pitch | Month 4-6 |
| ₹1L MRR | Monthly recurring revenue | Month 6-12 |

---

## Decision Log

| Date | Decision | Reason |
|------|----------|--------|
| 2026-04 | Strip to 4 core modes (Reply, Correct, Professional, Email) | Too many modes dilute UX. Focus on what professionals actually use daily. |
| 2026-04 | All modes preserve input language (except Correct) | Indians text in Hinglish/Hindi. Forcing English output kills authenticity. |
| 2026-04 | Move clipboard/templates out of main keyboard UI | Cluttered the AI bar, caused button overlap. Features are useful but secondary. |
| 2026-04 | Price at ₹29/mo, not ₹79 or ₹149 | India pricing reality. ₹29 is impulse, ₹79+ triggers deliberation. |
| 2026-04 | Prioritize PROCESS_TEXT over keyboard IME | Lower permission friction. Users can keep Gboard AND use SnapText. |
| 2026-04 | No watermarks, no share-to-grow mechanics | Indian users hate watermarks. Nobody shares their private replies. |
| 2026-04 | LinkedIn as primary distribution channel | Target audience (Indian professionals) lives on LinkedIn. Zero-cost organic reach. |
