const puppeteer = require('puppeteer');
(async () => {
  const browser = await puppeteer.launch({ args: ['--no-sandbox', '--disable-setuid-sandbox'] });
  const page = await browser.newPage();
  await page.goto('https://substack.com/@teeashby/note/p-195468760', { waitUntil: 'networkidle2' });
  // Find the menu button
  const buttons = await page.$$('button');
  for (let btn of buttons) {
    const text = await page.evaluate(el => el.innerText, btn);
    if (text.includes('Share') || text.includes('...')) {
        await btn.click();
        await page.waitForTimeout(1000);
    }
  }
  const content = await page.content();
  const embedCodeMatch = content.match(/<iframe.*?src=".*?".*?>.*?<\/iframe>/i);
  if (embedCodeMatch) {
      console.log("Found iframe:", embedCodeMatch[0]);
  } else {
      console.log("No iframe found. Page contains 'embed':", content.includes('embed'));
      const htmlMatches = content.match(/<script.*?src=".*?embed.*?">/g);
      console.log("Embed scripts:", htmlMatches);
      
      const customLengthMatches = content.match(/data-substack-custom-length/g);
      console.log("Substack custom length tags:", customLengthMatches);
  }
  await browser.close();
})();
