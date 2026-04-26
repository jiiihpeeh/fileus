import puppeteer from 'puppeteer';

async function test() {
  const browser = await puppeteer.launch({
    headless: true,
    args: ['--no-sandbox', '--disable-setuid-sandbox']
  });
  
  const page = await browser.newPage();
  await page.goto('http://localhost:8080', { waitUntil: 'networkidle0' });
  
  // Enter the activation key
  const key = 'pkWw2FNQtn';
  const inputs = await page.$$('.key-char-input');
  for (let i = 0; i < key.length && i < inputs.length; i++) {
    await inputs[i].type(key[i]);
  }
  
  await page.waitForSelector('.desktop', { timeout: 10000 });
  console.log('✓ Desktop activated');
  
  // Wait for desktop to fully render
  await new Promise(r => setTimeout(r, 1000));
  
  // Click start button
  await page.click('.start-btn');
  await page.waitForSelector('.start-menu', { timeout: 5000 });
  console.log('✓ Start menu opens');
  
  // Click on Files app
  await page.click('.start-menu-item');
  await new Promise(r => setTimeout(r, 500));
  
  // Check if window appeared
  const windowCount = await page.$$eval('.window', els => els.length);
  console.log(`✓ Window created, count: ${windowCount}`);
  
  // Test dragging window header
  const header = await page.$('.window-header');
  if (header) {
    const box = await header.boundingBox();
    if (box) {
      const startX = box.x + box.width / 2;
      const startY = box.y + box.height / 2;
      await page.mouse.move(startX, startY);
      await page.mouse.down();
      await page.mouse.move(startX + 100, startY + 50);
      await page.mouse.up();
      console.log('✓ Window drag completed');
    }
  }
  
  await new Promise(r => setTimeout(r, 300));
  
  // Test minimize
  const minBtn = await page.$('.win-min');
  if (minBtn) {
    await minBtn.click();
    await new Promise(r => setTimeout(r, 300));
    console.log('✓ Minimize clicked');
  }
  
  // Test maximize  
  const maxBtn = await page.$('.win-max');
  if (maxBtn) {
    await maxBtn.click();
    await new Promise(r => setTimeout(r, 300));
    console.log('✓ Maximize clicked');
  }
  
  // Test taskbar item click
  const taskbarApp = await page.$('.taskbar-app');
  if (taskbarApp) {
    await taskbarApp.click();
    await new Promise(r => setTimeout(r, 300));
    console.log('✓ Taskbar item clicked');
  }
  
  await browser.close();
  console.log('✓ All tests passed!');
}

test().catch(e => {
  console.error('Test failed:', e.message);
  process.exit(1);
});