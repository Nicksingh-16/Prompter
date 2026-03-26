const https = require('https');

const KEYS = [
    'AIzaSyAGofcknbeT-x1cj7PYOaj-vwlzNkUiaBw',
    'AIzaSyAqJmwPeB8dZTHUtpuWDVGBsm1ihUjyH48',
    'AIzaSyDmVCJNF1SB1kPXdyt53Tf2zVu-9vyeHio'
];

function testKey(key, index) {
    return new Promise((resolve) => {
        console.log(`\n--- Testing Key ${index + 1}: ${key.substring(0, 10)}... ---`);
        // Testing gemini-1.5-flash as it's the most common candidate for rate limits
        const url = `https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent?key=${key}`;
        const data = JSON.stringify({ contents: [{ parts: [{ text: "Hi" }] }] });

        const options = {
            method: 'POST',
            headers: { 'Content-Type': 'application/json', 'Content-Length': Buffer.byteLength(data) }
        };

        const req = https.request(url, options, (res) => {
            let body = '';
            res.on('data', (chunk) => body += chunk);
            res.on('end', () => {
                if (res.statusCode === 200) {
                    try {
                        const parsed = JSON.parse(body);
                        console.log(`✅ Success! Response: ${parsed.candidates[0].content.parts[0].text}`);
                    } catch (e) {
                        console.log("✅ Success! (Response received but could not parse JSON)");
                    }
                } else {
                    console.error(`❌ FAILED (Status ${res.statusCode})`);
                    console.error(body);
                }
                resolve();
            });
        });

        req.on('error', (e) => {
            console.error(`❌ Network Error: ${e.message}`);
            resolve();
        });

        req.write(data);
        req.end();
    });
}

function run(index = 0) {
    if (index >= KEYS.length) {
        console.log("\nDiagnostic Complete.");
        return;
    }
    testKey(KEYS[index], index).then(() => run(index + 1));
}

console.log("Starting Gemini Key Diagnostic (Zero dependencies)...");
run();
