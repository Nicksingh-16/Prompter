/**
 * SnapText Cloudflare Worker
 *
 * Handles:
 *  - Key rotation       (GEMINI_KEY_1 / GEMINI_KEY_2 / GEMINI_KEY_3 as Worker secrets)
 *  - Per-device daily usage cap via Cloudflare KV (USAGE namespace)
 *  - Streaming + non-streaming Gemini proxy
 *  - Optional Pro bypass via X-Pro-Secret header
 *
 * Deploy:
 *   1. npx wrangler login
 *   2. npx wrangler kv:namespace create USAGE  → paste the id into wrangler.toml
 *   3. npx wrangler secret put GEMINI_KEY_1    (paste key when prompted)
 *      npx wrangler secret put GEMINI_KEY_2
 *      npx wrangler secret put GEMINI_KEY_3
 *      npx wrangler secret put PRO_SECRET      (any random string for Pro users)
 *   4. npx wrangler deploy
 */

const DAILY_FREE_CAP = 20;   // free tier daily limit per device
const MODELS = [
    "gemini-2.0-flash",
    "gemini-1.5-flash",
];

export default {
    async fetch(request, env) {
        // ── CORS pre-flight ──────────────────────────────────────────────────
        if (request.method === "OPTIONS") {
            return cors(new Response(null, { status: 204 }));
        }

        const url = new URL(request.url);
        const path = url.pathname;

        // ── Usage check endpoint (GET /usage?device=xxx) ─────────────────────
        if (request.method === "GET" && path === "/usage") {
            const device = url.searchParams.get("device") || "unknown";
            const { used, cap } = await getUsage(env, device);
            return cors(json({ used, cap }));
        }

        // ── All other routes must be POST /generate or /classify ────────────
        if (request.method !== "POST") {
            return cors(json({ error: "Method not allowed" }, 405));
        }

        const device = request.headers.get("X-Device-ID") || "unknown";
        const isPro = request.headers.get("X-Pro-Secret") === env.PRO_SECRET;

        // ── Check daily cap (free users only) ───────────────────────────────
        if (!isPro) {
            const { used, cap } = await getUsage(env, device);
            if (used >= cap) {
                return cors(json({ error: "Daily limit reached", used, cap }, 429));
            }
        }

        // ── Parse request body ───────────────────────────────────────────────
        let body;
        try { body = await request.json(); }
        catch { return cors(json({ error: "Invalid JSON" }, 400)); }

        const { system_prompt, user_text, stream = false, max_tokens = 4096, temperature = 0.7 } = body;

        if (!user_text) return cors(json({ error: "user_text is required" }, 400));

        const geminiBody = {
            contents: [{
                parts: [{ text: system_prompt ? `${system_prompt}\n\nInput: ${user_text}` : user_text }]
            }],
            generationConfig: { temperature, maxOutputTokens: max_tokens },
        };

        // ── Key rotation + model fallback ────────────────────────────────────
        const keys = [env.GEMINI_KEY_1, env.GEMINI_KEY_2, env.GEMINI_KEY_3].filter(Boolean);
        let lastErr = "No keys configured";

        for (const model of MODELS) {
            for (const key of keys) {
                const endpoint = stream ? "streamGenerateContent" : "generateContent";
                const sep = stream ? "&alt=sse" : "";
                const apiUrl = `https://generativelanguage.googleapis.com/v1beta/models/${model}:${endpoint}?key=${key}${sep}`;

                let resp;
                try { resp = await fetch(apiUrl, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(geminiBody) }); }
                catch (e) { lastErr = e.message; continue; }

                if (resp.status === 429) { lastErr = "Rate limited"; continue; }

                if (!resp.ok) {
                    lastErr = `Gemini error ${resp.status}`;
                    continue;
                }

                // ── Success: bump usage and proxy the response ───────────────────
                if (!isPro) await incrementUsage(env, device);

                if (stream) {
                    // Pass SSE stream straight through
                    return cors(new Response(resp.body, {
                        status: 200,
                        headers: { "Content-Type": "text/event-stream", "Cache-Control": "no-cache" }
                    }));
                }

                const data = await resp.json();
                return cors(json(data));
            }
        }

        return cors(json({ error: lastErr }, 503));
    }
};

// ── KV usage helpers ──────────────────────────────────────────────────────────

function todayKey(device) {
    const d = new Date();
    return `usage:${device}:${d.getUTCFullYear()}-${d.getUTCMonth() + 1}-${d.getUTCDate()}`;
}

async function getUsage(env, device) {
    const val = await env.USAGE.get(todayKey(device));
    return { used: val ? parseInt(val) : 0, cap: DAILY_FREE_CAP };
}

async function incrementUsage(env, device) {
    const key = todayKey(device);
    const val = await env.USAGE.get(key);
    const n = val ? parseInt(val) + 1 : 1;
    // expire at midnight UTC+0 next day (86400s max)
    await env.USAGE.put(key, String(n), { expirationTtl: 86400 });
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function json(obj, status = 200) {
    return new Response(JSON.stringify(obj), {
        status,
        headers: { "Content-Type": "application/json" }
    });
}

function cors(response) {
    const r = new Response(response.body, response);
    r.headers.set("Access-Control-Allow-Origin", "*");
    r.headers.set("Access-Control-Allow-Headers", "Content-Type, X-Device-ID, X-Pro-Secret");
    r.headers.set("Access-Control-Allow-Methods", "GET, POST, OPTIONS");
    return r;
}
