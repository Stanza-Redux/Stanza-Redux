// Run: cargo run --quiet --example export_book -- resource/assets/Alice.epub > /tmp/stanza-book.json
// PLAYWRIGHT_ROOT=/path/with/node_modules node --test tests/reader.mjs
import { test } from 'node:test';
import { installResourceFixture } from './reader-resources.mjs';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { createRequire } from 'node:module';
import path from 'node:path';
const { chromium, webkit } = createRequire(path.join(process.env.PLAYWRIGHT_ROOT || process.cwd(), 'anchor.cjs'))('playwright');
const root = path.resolve('resource/assets/reader');
const book = JSON.parse(await readFile(process.env.STANZA_TEST_BOOK || '/tmp/stanza-book.json', 'utf8'));
for (const engine of [chromium, webkit]) test(`${engine.name()}: cover, pagination, columns, restoration and script isolation`, async () => {
  const server = createServer(async (req,res) => {
    try {
      const name = req.url.split('?')[0] === '/' ? '/index.html' : req.url.split('?')[0];
      if (name.includes('..')) throw Error('path');
      const data = await readFile(root + name);
      res.setHeader('Content-Type', name.endsWith('.js') ? 'text/javascript' : name.endsWith('.css') ? 'text/css' : 'text/html');
      res.end(data);
    } catch { res.writeHead(404).end(); }
  });
  await new Promise(r=>server.listen(0,'127.0.0.1',r));
  const browser = await engine.launch();
  try {
    const page = await browser.newPage({viewport:{width:1100,height:760}});
    await installResourceFixture(page);
    await page.goto(`http://127.0.0.1:${server.address().port}/`);
    await page.evaluate(book => fixtureLoad(book,{size:20,line:1.6,margin:24,theme:0,columns:1,font:0},{},{locale:'en', appTitle:'Reader fixture', loading:'Loading fixture', appearanceGlyph:'Aa', close:'Library fixture', appearance:'Settings fixture', done:'Done fixture', hint:'Gesture fixture', previous:'Previous fixture', next:'Next fixture', contents:'Contents fixture', position:'{chapter}/{chapters} · {page}/{pages}'}),book);
    await page.waitForFunction(()=>stanza.state().ready);
    assert.equal(await page.evaluate(()=>document.querySelector('#book').contentDocument.querySelector('image').getAttribute('xlink:href').startsWith('blob:')),false);
    await page.evaluate(()=>stanza.next());
    await page.waitForFunction(()=>stanza.state().ready&&stanza.state().chapter===1);
    assert.match(await page.evaluate(()=>stanza.state().text),/Gutenberg/);
    assert.equal(await page.locator('iframe').count(),1);
    await page.evaluate(()=>stanza.goToChapter(2));
    await page.waitForFunction(()=>stanza.state().ready);
    const before = await page.evaluate(()=>stanza.state());
    assert.ok(before.pages>1);
    await page.evaluate(()=>stanza.next());
    await page.waitForFunction(()=>stanza.state().ready&&!stanza.state().animating);
    assert.equal(await page.evaluate(()=>stanza.state().page),before.page+1);
    assert.ok(await page.evaluate(()=>Math.abs(document.querySelector('#book').contentWindow.scrollX)>100));
    await page.evaluate(()=>stanza.configure({size:24,line:1.7,margin:32,theme:2,columns:2,font:1}));
    await page.waitForFunction(()=>stanza.state().ready);
    assert.equal(await page.evaluate(()=>getComputedStyle(document.querySelector('#book').contentDocument.documentElement).columnCount),'2');
    await page.setViewportSize({width:390,height:700});
    await page.waitForTimeout(300);
    await page.waitForFunction(()=>stanza.state().ready);
    assert.equal(await page.evaluate(()=>getComputedStyle(document.querySelector('#book').contentDocument.documentElement).columnCount),'1');
    assert.equal(await page.evaluate(()=>getComputedStyle(document.querySelector('#book').contentDocument.documentElement).paddingLeft),'32px');
    await page.evaluate(()=>stanza.configure({size:24,line:1.7,margin:32,theme:3,paper:0x102030,ink:0xefdecf,columns:1,font:1}));
    await page.waitForFunction(()=>stanza.state().ready);
    assert.equal(await page.evaluate(()=>getComputedStyle(document.querySelector('#book').contentDocument.documentElement).backgroundColor),'rgb(16, 32, 48)');
    assert.equal(await page.evaluate(()=>getComputedStyle(document.querySelector('#book').contentDocument.body).color),'rgb(239, 222, 207)');

    const stable=await page.evaluate(()=>stanza.state());
    await page.mouse.click(195,350);
    assert.equal(await page.evaluate(()=>stanza.state().controls),false);
    assert.equal(await page.evaluate(()=>stanza.state().height),stable.height);
    await page.mouse.click(370,350);
    await page.waitForFunction(()=>stanza.state().ready&&!stanza.state().animating);
    assert.equal(await page.evaluate(()=>stanza.state().page),stable.page+1);
    const swipePage=await page.evaluate(()=>stanza.state().page);
    await page.evaluate(()=>{
      const d=document.querySelector('#book').contentDocument;
      for(const [type,x] of [['pointerdown',310],['pointermove',200],['pointerup',150]])
        d.body.dispatchEvent(new PointerEvent(type,{bubbles:true,cancelable:true,pointerId:7,isPrimary:true,pointerType:'touch',clientX:x,clientY:300}));
    });
    await page.waitForFunction(()=>stanza.state().ready&&!stanza.state().animating);
    assert.equal(await page.evaluate(()=>stanza.state().page),swipePage+1);
    const cancelPage=await page.evaluate(()=>stanza.state().page);
    await page.evaluate(()=>{
      const d=document.querySelector('#book').contentDocument;
      for(const [type,x] of [['pointerdown',310],['pointermove',230],['pointercancel',230]])
        d.body.dispatchEvent(new PointerEvent(type,{bubbles:true,cancelable:true,pointerId:8,isPrimary:true,pointerType:'touch',clientX:x,clientY:300}));
    });
    await page.waitForFunction(()=>stanza.state().ready&&!stanza.state().animating);
    assert.equal(await page.evaluate(()=>stanza.state().page),cancelPage);
    const wheelPage=await page.evaluate(()=>stanza.state().page);
    await page.evaluate(()=>{const d=document.querySelector('#book').contentDocument;d.body.dispatchEvent(new WheelEvent('wheel',{bubbles:true,cancelable:true,deltaX:90}));d.body.dispatchEvent(new WheelEvent('wheel',{bubbles:true,cancelable:true,deltaX:90}));});
    await page.waitForFunction(()=>stanza.state().ready&&!stanza.state().animating);
    assert.equal(await page.evaluate(()=>stanza.state().page),wheelPage+1);
    await page.waitForTimeout(270);
    await page.evaluate(()=>document.querySelector('#book').contentDocument.body.dispatchEvent(new WheelEvent('wheel',{bubbles:true,cancelable:true,deltaX:3,deltaY:140})));
    assert.equal(await page.evaluate(()=>stanza.state().page),wheelPage+1);
    await page.evaluate(()=>document.querySelector('#book').contentDocument.body.dispatchEvent(new KeyboardEvent('keydown',{bubbles:true,key:'ArrowLeft'})));
    await page.waitForFunction(()=>stanza.state().ready&&!stanza.state().animating);
    assert.equal(await page.evaluate(()=>stanza.state().page),wheelPage);
    await page.emulateMedia({reducedMotion:'reduce'});
    await page.evaluate(()=>stanza.next());
    assert.equal(await page.evaluate(()=>stanza.state().animating),false);
    const malicious=structuredClone(book), first=malicious.chapters[0].path;
    const dir=path.posix.dirname(first), style=p=>`${dir==='.'?'':dir+'/'}${p}`;
    malicious.assets[style('stanza-a.css')]={mime:'text/css',data:Buffer.from('@import url("stanza-b.css");').toString('base64')};
    malicious.assets[style('stanza-b.css')]={mime:'text/css',data:Buffer.from('p {letter-spacing:3px!important}').toString('base64')};
    malicious.assets[first].data=Buffer.from('<html><head><link rel=stylesheet href="stanza-a.css"></head><body><script>parent.compromised=true</script><p onclick="parent.compromised=true">Safe text</p><iframe src="https://invalid.test"></iframe></body></html>').toString('base64');
    await page.evaluate(b=>fixtureLoad(b,{size:20},{}),malicious);
    await page.waitForFunction(()=>stanza.state().ready);
    // CSP also blocks scripts if a future sanitizer change accidentally leaves one behind.
    await page.evaluate(()=>{const d=document.querySelector('#book').contentDocument;const s=d.createElement('script');s.textContent='parent.compromised=true';d.body.append(s);s.remove()});
    await page.waitForTimeout(50);
    assert.equal(await page.evaluate(()=>window.compromised),undefined);
    assert.equal(await page.evaluate(()=>getComputedStyle(document.querySelector('#book').contentDocument.querySelector('p')).letterSpacing),'3px');
    assert.equal(await page.evaluate(()=>document.querySelector('#book').contentDocument.querySelectorAll('script,iframe,[onclick]').length),0);
  } finally { await browser.close(); await new Promise(r=>server.close(r)); }
});


// Synthetic short chapters isolate focus handoff from pagination and book content.
// The outer iframe models day-piece-webview on web-dom; keys must follow real focus.
for (const engine of [chromium, webkit]) test(`${engine.name()}: nested reader retains keyboard focus across chapters`, async () => {
  const server = createServer(async (req, res) => {
    try {
      if (req.url === '/host') {
        res.setHeader('Content-Type', 'text/html');
        res.end('<input id="outside" aria-label="Host input fixture"><iframe id="reader" src="/index.html" style="display:block;width:900px;height:650px"></iframe>');
        return;
      }
      if (req.url.includes('..')) throw Error('path');
      res.setHeader('Content-Type', req.url.endsWith('.js') ? 'text/javascript' : 'text/html');
      res.end(await readFile(root + req.url));
    } catch { res.writeHead(404).end(); }
  });
  await new Promise(r => server.listen(0, '127.0.0.1', r));
  const browser = await engine.launch();
  try {
    const page = await browser.newPage({viewport:{width:1100,height:800}});
    await installResourceFixture(page);
    await page.goto(`http://127.0.0.1:${server.address().port}/host`);
    const reader = page.frames().find(f => f.url().endsWith('/index.html'));
    const chapters = Array.from({length:4}, (_, i) => ({path:`chapter-${i}.html`,title:`Chapter fixture ${i}`}));
    const fixture = {title:'Focus fixture',language:'en',chapters,toc:chapters,assets:Object.fromEntries(chapters.map((c,i) => [c.path, {
      mime:'application/xhtml+xml',data:Buffer.from(`<html><body><h1>${c.title}</h1><p>Short chapter fixture.</p><a href="chapter-${(i+1)%4}.html#section">Section link fixture</a><p id="section">Section fixture</p></body></html>`).toString('base64')
    }]))};
    await reader.evaluate(b => fixtureLoad(b,{size:20},{},{locale:'en',appTitle:'Focus fixture',appearanceGlyph:'Aa',close:'Library fixture',appearance:'Settings fixture',done:'Done fixture',previous:'Previous fixture',next:'Next fixture',contents:'Contents fixture',position:'{chapter}/{chapters}'}),fixture);
    const settled = index => reader.waitForFunction(i => stanza.state().ready && !stanza.state().animating && stanza.state().chapter === i, index, {timeout:3000});
    await settled(0);
    assert.equal(await reader.evaluate(() => stanza.state().pages),1);
    // One initial click only: subsequent page turns must not need focus repair.
    await page.frameLocator('#reader').frameLocator('#book').locator('h1').click();
    for (const motion of ['no-preference','reduce']) {
      await page.emulateMedia({reducedMotion:motion});
      for (const index of [1,2,3]) { await page.keyboard.press('ArrowRight'); await settled(index); }
      for (const index of [2,1,0]) { await page.keyboard.press('ArrowLeft'); await settled(index); }
      await page.keyboard.press('PageDown'); await settled(1);
      await page.keyboard.press('PageUp'); await settled(0);
    }
    // Section links replace srcdoc in the same iframe, without a slide.
    await page.frameLocator('#reader').frameLocator('#book').locator('a').click();
    await settled(1);
    await page.keyboard.press('ArrowRight'); await settled(2);
    // Relayout can replace a chapter while the reader owns keyboard focus.
    await reader.evaluate(() => stanza.configure({size:22,columns:2}));
    await settled(2);
    await page.keyboard.press('ArrowLeft'); await settled(1);
    // Async chapter completion must not steal focus from the surrounding app.
    await page.emulateMedia({reducedMotion:'no-preference'});
    await page.keyboard.press('ArrowRight');
    await page.locator('#outside').focus();
    await settled(2);
    await page.keyboard.type('Host typing fixture');
    assert.equal(await page.locator('#outside').inputValue(),'Host typing fixture');
    await reader.evaluate(() => stanza.goToChapter(0));
    await settled(0);
    assert.equal(await page.locator('#outside').evaluate(el => el === document.activeElement),true);
  } finally { await browser.close(); await new Promise(r => server.close(r)); }
});

for (const engine of [chromium,webkit]) test(`${engine.name()}: sections load through relative URLs on demand, cancel and never create blobs`,async()=>{
 const server=createServer(async(req,res)=>{try{const name=req.url==='/'?'/index.html':req.url;if(name.includes('..'))throw Error();res.setHeader('Content-Type',name.endsWith('.js')?'text/javascript':'text/html');res.end(await readFile(root+name))}catch{res.writeHead(404).end()}});
 await new Promise(r=>server.listen(0,'127.0.0.1',r));
 const browser=await engine.launch();
 try{
  const page=await browser.newPage();await installResourceFixture(page);
  await page.addInitScript(()=>{
   const create=URL.createObjectURL.bind(URL),revoke=URL.revokeObjectURL.bind(URL);window.liveResourceUrls=new Set();
   URL.createObjectURL=blob=>{const u=create(blob);liveResourceUrls.add(u);return u};
   URL.revokeObjectURL=u=>{liveResourceUrls.delete(u);revoke(u)};
  });
  await page.goto(`http://127.0.0.1:${server.address().port}/`);
  const chapters=Array.from({length:60},(_,i)=>({path:`part-${i}.xhtml`,title:`Synthetic section ${i}`}));
  const encode=s=>Buffer.from(s).toString('base64');
  const fixture={id:'lazy-fixture',title:'Lazy fixture',language:'en',chapters,toc:chapters,assets:{
   ...Object.fromEntries(chapters.map((c,i)=>[c.path,{mime:'application/xhtml+xml',data:encode(`<html><head><link rel="stylesheet" href="a.css"/></head><body><h1>Section ${i}</h1>${i===1?'<!--'+'x'.repeat(500000)+'-->':''}</body></html>`)}])),
   'a.css':{mime:'text/css',data:encode('@import url("b.css");h1{color:rgb(12,34,56)}')},
   'b.css':{mime:'text/css',data:encode('@import url("a.css");p{color:blue}')},
   'unused.bin':{mime:'application/octet-stream',data:encode('unused resource fixture')}
  }};
  await page.evaluate(b=>fixtureLoad(b,{size:20},{},{locale:'en',appTitle:'Fixture',position:'{chapter}'}),fixture);
  const settled=i=>page.waitForFunction(i=>stanza.state().ready&&stanza.state().chapter===i,i);
  await settled(0);
  assert.deepEqual(await page.evaluate(()=>[...new Set(fixtureRequests.map(r=>r.path))].sort()),['a.css','b.css','part-0.xhtml']);
  assert.equal(await page.evaluate(()=>liveResourceUrls.size),0);
  await page.evaluate(()=>stanza.goToChapter(1));await settled(1);
  assert.equal(await page.evaluate(()=>fixtureRequests.filter(r=>r.path==='part-1.xhtml').length),1);
  assert.equal(await page.evaluate(()=>liveResourceUrls.size),0);
  await page.evaluate(()=>{window.fixtureDelay=60;stanza.goToChapter(2)});
  await page.waitForFunction(()=>fixtureRequests.some(r=>r.path==='part-2.xhtml'));
  await page.evaluate(()=>stanza.goToChapter(3));await settled(3);
  await page.waitForTimeout(150);
  assert.equal(await page.evaluate(()=>stanza.state().chapter),3);
  assert.equal(await page.evaluate(()=>liveResourceUrls.size),0);
  assert.equal(await page.evaluate(()=>fixtureRequests.some(r=>r.path==='unused.bin'||r.path==='part-59.xhtml')),false);
  await page.evaluate(()=>document.getElementById('close-reader').click());
  assert.equal(await page.evaluate(()=>liveResourceUrls.size),0);
 }finally{await browser.close();await new Promise(r=>server.close(r))}
});
