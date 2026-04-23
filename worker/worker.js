const CORS_HEADERS = {
    "Access-Control-Allow-Origin": "tauri://localhost",
    "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
    "Access-Control-Allow-Headers": "Content-Type, X-Device-ID, X-App-Secret",
};

// ── D1 schema (run once via wrangler d1 execute) ─────────────────────────────
// CREATE TABLE IF NOT EXISTS usage (
//   device_id TEXT NOT NULL,
//   date TEXT NOT NULL,
//   count INTEGER DEFAULT 0,
//   PRIMARY KEY (device_id, date)
// );
// CREATE TABLE IF NOT EXISTS events (
//   id INTEGER PRIMARY KEY AUTOINCREMENT,
//   device_id TEXT NOT NULL,
//   event TEXT NOT NULL,
//   mode TEXT,
//   app_context TEXT,
//   ts TEXT NOT NULL
// );

async function getUsageD1(db, deviceId, date) {
    const row = await db.prepare(
        "SELECT count FROM usage WHERE device_id = ? AND date = ?"
    ).bind(deviceId, date).first();
    return row ? row.count : 0;
}

async function incrementUsageD1(db, deviceId, date) {
    await db.prepare(
        "INSERT INTO usage (device_id, date, count) VALUES (?, ?, 1) " +
        "ON CONFLICT(device_id, date) DO UPDATE SET count = count + 1"
    ).bind(deviceId, date).run();
}

export default {
    async fetch(request, env) {
        const url = new URL(request.url);

        // ── CORS preflight ──────────────────────────────────────────────────
        if (request.method === "OPTIONS") {
            return new Response(null, { status: 204, headers: CORS_HEADERS });
        }

        // ── Auth ────────────────────────────────────────────────────────────
        const clientSecret = request.headers.get("X-App-Secret") || "";
        if (env.APP_SECRET && clientSecret !== env.APP_SECRET) {
            return new Response(JSON.stringify({ error: "Unauthorized" }), {
                status: 401,
                headers: { "Content-Type": "application/json", ...CORS_HEADERS }
            });
        }

        const deviceId = request.headers.get("X-Device-ID") || "unknown";
        const date = new Date().toISOString().split("T")[0];

        // ── GET /usage ──────────────────────────────────────────────────────
        if (url.pathname === "/usage" || (request.method === "GET" && url.pathname.endsWith("/usage"))) {
            let used = 0;
            if (env.DB) {
                used = await getUsageD1(env.DB, deviceId, date);
            } else {
                used = parseInt(await env.USAGE.get(`usage:${deviceId}:${date}`) || "0");
            }
            return new Response(JSON.stringify({ used, cap: 20 }), {
                headers: { "Content-Type": "application/json", ...CORS_HEADERS }
            });
        }

        // ── POST /telemetry ─────────────────────────────────────────────────
        // Receives anonymous usage events — no text content, only mode + context
        if (url.pathname === "/telemetry" && request.method === "POST") {
            if (!env.DB) {
                return new Response(JSON.stringify({ ok: true }), {
                    headers: { "Content-Type": "application/json", ...CORS_HEADERS }
                });
            }
            let body = {};
            try { body = await request.json(); } catch (_) {}
            const event      = (body.event      || "unknown").slice(0, 32);
            const mode       = (body.mode       || "").slice(0, 32);
            const appContext = (body.app_context || "").slice(0, 64);
            const ts         = new Date().toISOString();
            await env.DB.prepare(
                "INSERT INTO events (device_id, event, mode, app_context, ts) VALUES (?, ?, ?, ?, ?)"
            ).bind(deviceId, event, mode, appContext, ts).run();
            return new Response(JSON.stringify({ ok: true }), {
                headers: { "Content-Type": "application/json", ...CORS_HEADERS }
            });
        }

        // ── GET /analytics ──────────────────────────────────────────────────
        // Aggregate stats — mode distribution, active devices, top app contexts
        if (url.pathname === "/analytics" && request.method === "GET") {
            if (!env.DB) {
                return new Response(JSON.stringify({ error: "D1 not configured" }), {
                    status: 503, headers: { "Content-Type": "application/json", ...CORS_HEADERS }
                });
            }
            const [modeRows, dauRows, contextRows] = await Promise.all([
                env.DB.prepare(
                    "SELECT mode, COUNT(*) as cnt FROM events WHERE event='transform' AND ts >= date('now','-7 days') GROUP BY mode ORDER BY cnt DESC"
                ).all(),
                env.DB.prepare(
                    "SELECT COUNT(DISTINCT device_id) as dau FROM usage WHERE date >= date('now','-7 days')"
                ).first(),
                env.DB.prepare(
                    "SELECT app_context, COUNT(*) as cnt FROM events WHERE event='transform' AND ts >= date('now','-7 days') GROUP BY app_context ORDER BY cnt DESC LIMIT 5"
                ).all(),
            ]);
            return new Response(JSON.stringify({
                period: "7d",
                dau: dauRows ? dauRows.dau : 0,
                mode_distribution: modeRows.results,
                top_contexts: contextRows.results,
            }), { headers: { "Content-Type": "application/json", ...CORS_HEADERS } });
        }

        // ── POST /generate ──────────────────────────────────────────────────
        if (request.method === "POST" && url.pathname !== "/embed" && url.pathname !== "/telemetry") {
            const ct = request.headers.get("content-type") || "";
            if (!ct.includes("application/json")) {
                return new Response(JSON.stringify({ error: "Bad Request" }), {
                    status: 400, headers: { "Content-Type": "application/json", ...CORS_HEADERS }
                });
            }
            const contentLength = parseInt(request.headers.get("content-length") || "0");
            if (contentLength > 10240) {
                return new Response(JSON.stringify({ error: "Payload too large" }), {
                    status: 413, headers: { "Content-Type": "application/json", ...CORS_HEADERS }
                });
            }

            const body = await request.json();
            const { system_prompt, user_text, stream, model: reqModel, max_tokens, temperature, thinking_budget } = body;

            // Check daily cap (D1 preferred, KV fallback)
            let used = 0;
            if (env.DB) {
                used = await getUsageD1(env.DB, deviceId, date);
            } else {
                used = parseInt(await env.USAGE.get(`usage:${deviceId}:${date}`) || "0");
            }
            if (used >= 20) {
                return new Response(JSON.stringify({ error: "Daily limit reached (20/20). Resets at midnight." }), {
                    status: 429, headers: CORS_HEADERS
                });
            }

            const availableKeys = [env.GEMINI_KEY_1, env.GEMINI_KEY_2, env.GEMINI_KEY_3].filter(Boolean);
            if (availableKeys.length === 0) return new Response("No API keys configured", { status: 500 });
            const shuffledKeys = availableKeys.sort(() => Math.random() - 0.5);

            const model = reqModel || env.MODEL || "gemini-2.5-flash";
            const resolvedBudget = typeof thinking_budget === "number" ? thinking_budget : 0;
            const geminiBody = {
                systemInstruction: { parts: [{ text: system_prompt || "" }] },
                contents: [{ role: "user", parts: [{ text: user_text }] }],
                generationConfig: {
                    temperature: typeof temperature === "number" ? temperature : 0.5,
                    maxOutputTokens: typeof max_tokens === "number" && max_tokens > 0 ? Math.min(max_tokens, 8192) : 800,
                    thinkingConfig: { thinkingBudget: resolvedBudget }
                }
            };

            // Primary: gemini-2.0-flash (15 RPM free tier, stable).
            // Upgrade: gemini-2.5-flash if explicitly requested and keys allow.
            // This order prevents the 5 RPM experimental limit on 2.5 Flash from blocking all users.
            const STABLE_MODEL = "gemini-2.0-flash";
            const modelsToTry = model === "gemini-2.5-flash"
                ? [STABLE_MODEL, model]   // try stable first, 2.5 as bonus
                : [model, STABLE_MODEL];  // explicit model request respected, stable as fallback

            let lastError = null;
            for (const tryModel of modelsToTry) {
                // Gemini 2.0 Flash does not support thinkingConfig — strip it to avoid 400.
                const bodyForModel = tryModel.includes("2.0")
                    ? { ...geminiBody, generationConfig: { ...geminiBody.generationConfig, thinkingConfig: { thinkingBudget: 0 } } }
                    : geminiBody;

                let nonRetryable = false;
                for (const key of shuffledKeys) {
                    const endpoint = stream ? "streamGenerateContent" : "generateContent";
                    const geminiUrl = `https://generativelanguage.googleapis.com/v1beta/models/${tryModel}:${endpoint}?key=${key}${stream ? "&alt=sse" : ""}`;
                    const res = await fetch(geminiUrl, {
                        method: "POST",
                        headers: { "Content-Type": "application/json" },
                        body: JSON.stringify(bodyForModel)
                    });

                    if (res.ok) {
                        // Increment usage counter
                        if (env.DB) {
                            await incrementUsageD1(env.DB, deviceId, date);
                        } else {
                            await env.USAGE.put(`usage:${deviceId}:${date}`, (used + 1).toString(), { expirationTtl: 86400 });
                        }
                        const merged = new Headers(res.headers);
                        for (const [k, v] of Object.entries(CORS_HEADERS)) merged.set(k, v);
                        return new Response(res.body, { status: res.status, headers: merged });
                    }

                    const errStatus = res.status;
                    const errText = await res.text();
                    lastError = { status: errStatus, body: errText };
                    if (errStatus === 429 || errStatus === 503) {
                        console.log(`Key ${key.substring(0, 6)}... got ${errStatus} on ${tryModel}. Trying next...`);
                        continue;
                    }
                    nonRetryable = true;
                    break; // Hard error (401, 400, etc.) — don't try other keys or models
                }
                if (nonRetryable) break;
                // All keys transient-failed on this model — try fallback model if available
            }

            const displayError = lastError
                ? `Gemini error ${lastError.status}: ${lastError.body}`
                : "All keys exhausted or rate limited.";
            return new Response(JSON.stringify({ error: displayError }), {
                status: 503, headers: { "Content-Type": "application/json", ...CORS_HEADERS }
            });
        }

        // ── POST /embed ─────────────────────────────────────────────────────
        if (url.pathname === "/embed" && request.method === "POST") {
            const body = await request.json();
            const { text } = body;
            if (!text || typeof text !== "string" || text.length > 5000) {
                return new Response(JSON.stringify({ error: "Bad Request" }), {
                    status: 400, headers: { "Content-Type": "application/json", ...CORS_HEADERS }
                });
            }
            const availableKeys = [env.GEMINI_KEY_1, env.GEMINI_KEY_2, env.GEMINI_KEY_3].filter(Boolean);
            if (availableKeys.length === 0) return new Response("No API keys", { status: 500, headers: CORS_HEADERS });
            const key = availableKeys[Math.floor(Math.random() * availableKeys.length)];
            const embedUrl = `https://generativelanguage.googleapis.com/v1beta/models/text-embedding-004:embedContent?key=${key}`;
            const res = await fetch(embedUrl, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ model: "models/text-embedding-004", content: { parts: [{ text }] } })
            });
            const merged = new Headers(res.headers);
            for (const [k, v] of Object.entries(CORS_HEADERS)) merged.set(k, v);
            return new Response(res.body, { status: res.status, headers: merged });
        }

        return new Response("Not Found", { status: 404, headers: CORS_HEADERS });
    }
};
