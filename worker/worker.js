export default {
    async fetch(request, env) {
        const url = new URL(request.url);
        const deviceId = request.headers.get("X-Device-ID") || "unknown";
        const date = new Date().toISOString().split("T")[0];
        const usageKey = `usage:${deviceId}:${date}`;

        // ── GET /usage ──────────────────────────────────────────────────────
        if (url.pathname === "/usage" || (request.method === "GET" && url.pathname.endsWith("/usage"))) {
            const used = parseInt(await env.USAGE.get(usageKey) || "0");
            return new Response(JSON.stringify({ used, cap: 20 }), {
                headers: { "Content-Type": "application/json" }
            });
        }

        // ── POST /generate ──────────────────────────────────────────────────
        if (request.method === "POST") {
            const body = await request.json();
            const { system_prompt, user_text, stream, model: reqModel } = body;

            // 1. Check daily device cap
            const used = parseInt(await env.USAGE.get(usageKey) || "0");
            if (used >= 20) {
                return new Response(JSON.stringify({ error: "Daily limit reached (20/20). Resets at midnight." }), { status: 429 });
            }

            // 2. Resolve keys
            const availableKeys = [env.GEMINI_KEY_1, env.GEMINI_KEY_2, env.GEMINI_KEY_3].filter(Boolean);
            if (availableKeys.length === 0) return new Response("No API keys configured", { status: 500 });

            // 3. Shuffle keys for true rotation
            const shuffledKeys = availableKeys.sort(() => Math.random() - 0.5);

            const model = reqModel || env.MODEL || "gemini-2.0-flash";
            const geminiBody = {
                contents: [{ parts: [{ text: `${system_prompt}\n\n${user_text}` }] }],
                generationConfig: { temperature: 0.7, maxOutputTokens: 4096 }
            };

            // 4. Retry Loop (Failover)
            // If one key hits 429, try the next one immediately.
            let lastError = null;
            for (const key of shuffledKeys) {
                const geminiUrl = `https://generativelanguage.googleapis.com/v1beta/models/${model}:${stream ? "streamGenerateContent" : "generateContent"}?key=${key}${stream ? "&alt=sse" : ""}`;

                const res = await fetch(geminiUrl, {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify(geminiBody)
                });

                if (res.ok) {
                    // Success! Increment usage and return
                    await env.USAGE.put(usageKey, (used + 1).toString(), { expirationTtl: 86400 });
                    return new Response(res.body, { headers: res.headers });
                }

                const errStatus = res.status;
                const errText = await res.text();
                lastError = { status: errStatus, body: errText };

                // If it's a 429 (Rate Limit / Quota), try the next key
                if (errStatus === 429) {
                    console.log(`Key ${key.substring(0, 6)}... rate limited. Retrying...`);
                    continue;
                }

                // If it's some other non-retryable error (e.g. 400 Bad Request), break and show it
                break;
            }

            // If we're here, all retries failed or we hit a non-retryable error
            const displayError = lastError
                ? `Gemini error ${lastError.status}: ${lastError.body}`
                : "All keys exhausted or rate limited.";

            return new Response(JSON.stringify({ error: displayError }), { status: 503 });
        }

        return new Response("Not Found", { status: 404 });
    }
};
