const { chromium } = require('playwright');
const path = require('path');
const fs = require('fs');

(async () => {
  // Clean up any leftover webm files
  fs.readdirSync(__dirname).filter(f => f.endsWith('.webm')).forEach(f => {
    try { fs.unlinkSync(path.join(__dirname, f)); } catch(e) {}
  });

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

  console.log('Recording 66 seconds...');
  await page.waitForTimeout(66000);

  console.log('Finalizing...');
  const videoPath = await page.video().path();
  await ctx.close();
  await browser.close();

  // Wait for file handle to release
  await new Promise(r => setTimeout(r, 3000));

  const final = path.join(__dirname, 'snaptext_promo.webm');
  fs.renameSync(videoPath, final);

  console.log('\n✅ Done! Video saved to:');
  console.log('   ' + final);
  console.log('\nTo convert to MP4 (if you have ffmpeg):');
  console.log('   ffmpeg -i snaptext_promo.webm -c:v libx264 -crf 18 snaptext_promo.mp4');
})();
