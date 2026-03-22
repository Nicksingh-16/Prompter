# AI Prompter (Prompter) 🤖🚀

**AI Prompter** is a high-performance, universal AI overlay designed to transform your workflow. Optimized for speed and security, it allows you to select text in *any* application and apply AI-powered transformations instantly with a single shortcut (**Alt+K**).

## ✨ Key Features
- **🚀 Gemini 2.5 Flash Engine**: Leverages the latest high-speed, low-latency AI for instantaneous text processing.
- **🛡️ Bulletproof Persistence**: Secure, SQLite-backed storage for API keys (obfuscated with machine-specific XOR) ensures you never have to re-onboard after a restart.
- **🎓 Intelligent Knowledge Mode**: Automatically detects when you're asking for help or instructions (e.g., Git, Database management) and switches to an educational, mentoring tone.
- **🖋️ Direct OS Injection**: Reliable "Insert" functionality that hides the overlay and simulates keyboard events to paste results directly into your target editor/browser.
- **🧠 Context-Aware Intent**: Multi-layered classification system (Regex + AI) that suggests the best mode (Email, Professional, Casual, Summarize, etc.) for your text.

## 🛠️ Tech Stack
- **Backend**: Rust (Tauri v2)
- **Frontend**: React, TypeScript, Framer Motion
- **Database**: SQLite (rusqlite)
- **AI**: Gemini 1.5/2.5 Flash API

## 🚀 Getting Started
1. `npm install`
2. `npm run tauri dev`
3. Enter your Gemini API key (from [AI Studio](https://aistudio.google.com/)).
4. Highlight some text, press **Alt+K**, and transform.

## 📝 License
MIT
