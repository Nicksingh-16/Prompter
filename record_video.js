const { chromium } = require('playwright');
const path = require('path');

(async () => {
  const browser = await chromium.launch({ headless: true });
  const ctx = await browser.newContext({
    viewport: { width: 1920, height: 1080 },
    recordVideo: {
      dir: path.join(__dirname),
      size: { width: 1920, height: 1080 },
    },
  });

  const page = await ctx.newPage();
  const filePath = 'file:///' + path.join(__dirname, 'video_animation.html').replace(/\\/g, '/');

  console.log('Opening animation...');
  await page.goto(filePath);

  // Wait for Google Fonts to load
  await page.waitForTimeout(2000);

  console.log('Recording 42 seconds...');
  await page.waitForTimeout(42000);

  console.log('Saving video...');
  await ctx.close();
  await browser.close();

  // The video file is saved to D:/ai_keyboard/ as a .webm
  const fs = require('fs');
  const files = fs.readdirSync(__dirname).filter(f => f.endsWith('.webm'));
  if (files.length) {
    const latest = files.sort().at(-1);
    const final = path.join(__dirname, 'snaptext_promo.webm');
    fs.renameSync(path.join(__dirname, latest), final);
    console.log('\n✅ Done! Video saved to:');
    console.log('   ' + final);
    console.log('\nTo convert to MP4 (if you have ffmpeg):');
    console.log('   ffmpeg -i snaptext_promo.webm -c:v libx264 -crf 18 snaptext_promo.mp4');
  } else {
    console.log('⚠️  No .webm file found — check if recording worked.');
  }
})();
