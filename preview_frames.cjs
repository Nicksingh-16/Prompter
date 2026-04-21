const { chromium } = require('playwright');
const path = require('path');
const fs = require('fs');

(async () => {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  await page.setViewportSize({ width: 1920, height: 1080 });

  const filePath = 'file:///' + path.join('D:/ai_keyboard', 'video_animation.html').replace(/\/g, '/');
  await page.goto(filePath);

  // Screenshots at key moments
  const shots = [
    { t: 100,   name: 'sc1_start' },
    { t: 3000,  name: 'sc1_mid' },
    { t: 5200,  name: 'sc1_end_counter' },
    { t: 7000,  name: 'sc2_whatsapp' },
    { t: 10500, name: 'sc2_pill' },
    { t: 13000, name: 'sc2_toast' },
    { t: 16000, name: 'sc2_injected' },
    { t: 21000, name: 'sc3_outlook' },
    { t: 24000, name: 'sc3_overlay_streaming' },
    { t: 29000, name: 'sc3_overlay_done' },
    { t: 33000, name: 'sc3_injected' },
    { t: 37000, name: 'sc4_vscode' },
    { t: 41000, name: 'sc4_pill' },
    { t: 44000, name: 'sc4_toast_done' },
    { t: 47000, name: 'sc4_result_overlay' },
    { t: 50000, name: 'sc5_cards' },
    { t: 57000, name: 'sc6_cta' },
  ];

  const outDir = path.join('D:/ai_keyboard', 'preview_frames');
  if (!fs.existsSync(outDir)) fs.mkdirSync(outDir);

  let prev = 0;
  for (const s of shots) {
    await page.waitForTimeout(s.t - prev);
    prev = s.t;
    const file = path.join(outDir, `${s.name}.png`);
    await page.screenshot({ path: file });
    console.log(`✓ ${s.name}.png @ ${s.t}ms`);
  }

  await browser.close();
  console.log('\nAll frames saved to D:/ai_keyboard/preview_frames/');
})();
