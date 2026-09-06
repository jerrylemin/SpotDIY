async (page) => {
  await page.evaluate(async () => {
    const React = (await import('/node_modules/.vite/deps/react.js')).default;
    const { createRoot } = (await import('/node_modules/.vite/deps/react-dom_client.js')).default;
    const { LyricsPanel } = await import('/src/components/lyrics/LyricsPanel.tsx');
    const host = document.createElement('div');
    document.querySelector('main').replaceChildren(host);
    const lines = ['The city settles into blue', 'A little light is coming through', 'We follow every passing sound', 'And let the evening slow us down', 'Another moment, yours and mine', 'We find our rhythm, line by line'];
    const doc = {trackId:'preview', source:'manual', syncKind:'timed', cues:lines.map((text,index)=>({startMs:index*4000,lines:[text],words:[]})), plainText:null, instrumental:false, editable:true, attribution:null};
    const root = createRoot(host);
    window.lyricsPreview = (positionMs) => {
      const panel = React.createElement(LyricsPanel,{document:doc,positionMs,durationMs:24000,onSeek:(position)=>window.lyricsPreview(position)});
      const header = React.createElement('h2',null,'Nightfall - Synthetic lyrics preview');
      root.render(React.createElement('div',{className:'lyrics-page'},React.createElement('section',{className:'lyrics-panel'},header,panel)));
    };
    window.lyricsPreview(9000);
  });
  await page.locator('.lyrics-cue-active').waitFor();
  await page.screenshot({path:'output/playwright/lyrics-redesign.png'});
}


