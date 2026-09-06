async (page) => {
  await page.locator('.lyrics-cue-active').waitFor();
  const viewport = page.locator('.lyrics-cue-list');
  await viewport.hover();
  await page.mouse.wheel(0, 120);
  await page.getByRole('button', {name:'Back to current line ↓'}).waitFor();
  await page.getByRole('button', {name:'Back to current line ↓'}).click();
  await page.evaluate(() => window.lyricsPreview(13000));
  await page.locator('.lyrics-cue-active').filter({hasText:'And let the evening slow us down'}).waitFor();
  await page.screenshot({path:'output/playwright/lyrics-redesign.png'});
  console.log(await page.evaluate(() => ({overflow:document.documentElement.scrollWidth > window.innerWidth, active:document.querySelector('.lyrics-cue-active').textContent, viewportHeight:document.querySelector('.lyrics-cue-list').clientHeight})));
}
