// Synthetic bookmark bridge; database durability is covered by the Rust storage test.
// PLAYWRIGHT_ROOT=/path/with/node_modules node --test tests/reader-affordances.mjs
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createServer} from 'node:http';
import {createRequire} from 'node:module';
import path from 'node:path';
import {installResourceFixture} from './reader-resources.mjs';
const {chromium,webkit}=createRequire(path.join(process.env.PLAYWRIGHT_ROOT||process.cwd(),'anchor.cjs'))('playwright');
const root=path.resolve('resource/assets/reader');
const book=JSON.parse(await readFile(process.env.STANZA_TEST_BOOK||'/tmp/stanza-book.json','utf8'));
for(const engine of [chromium,webkit])test(`${engine.name()}: bookmarks, seek/return, contents filtering and dialog focus`,async()=>{
 const server=createServer(async(req,res)=>{try{const name=req.url==='/'?'/index.html':req.url;if(name.includes('..'))throw Error();res.setHeader('Content-Type',name.endsWith('.js')?'text/javascript':'text/html');res.end(await readFile(root+name))}catch{res.writeHead(404).end()}});
 await new Promise(r=>server.listen(0,'127.0.0.1',r));const browser=await engine.launch();
 try{
  const page=await browser.newPage({viewport:{width:390,height:760}});await installResourceFixture(page);
  await page.addInitScript(()=>{
   window.marks=[];window.bookmarkEvents=0;
   window.addEventListener('click',e=>{
    const a=e.target.closest?.('a');if(!a?.href.startsWith('stanza://bookmark?'))return;
    const data=JSON.parse(new URL(a.href).searchParams.get('data'));bookmarkEvents++;
    if(data.remove)marks=marks.filter(b=>b.id!==data.remove);
    else marks.push({...data,id:`fixture-${marks.length}`,title:'Saved chapter fixture'});
    queueMicrotask(()=>stanza.bookmarks(marks));
   },true);
  });
  await page.goto(`http://127.0.0.1:${server.address().port}/`);
  await page.evaluate(book=>fixtureLoad(book,{size:20,margin:24,columns:1},{chapter:2},{locale:'en',position:'{chapter}/{chapters} · {page}/{pages}',contents:'Contents fixture',done:'Done fixture',bookmarkAdd:'Save fixture',bookmarkRemove:'Remove fixture',bookmarks:'Bookmarks fixture',bookmarksEmpty:'Empty fixture',chapterPosition:'Position fixture',returnPosition:'Return fixture',contentsSearch:'Find fixture',contentsEmpty:'No matches fixture',chapterPercent:'{percent} in fixture'}),book);
  const settled=()=>page.waitForFunction(()=>stanza.state().ready&&!stanza.state().animating);
  await settled();
  assert.equal(await page.locator('#bookmark').getAttribute('aria-pressed'),'false');
  await page.locator('#bookmark').click();
  await page.waitForFunction(()=>document.getElementById('bookmark').getAttribute('aria-pressed')==='true');
  assert.equal(await page.evaluate(()=>bookmarkEvents),1);
  const original=await page.evaluate(()=>stanza.state().page);
  await page.locator('#chapter-progress').evaluate(el=>{el.value='650';el.dispatchEvent(new Event('input'));el.dispatchEvent(new Event('change'))});
  await settled();assert.ok(await page.evaluate(()=>stanza.state().page)>original);
  assert.equal(await page.locator('#bookmark').getAttribute('aria-pressed'),'false');
  await page.locator('#return-position').click();await settled();
  assert.equal(await page.evaluate(()=>stanza.state().page),original);
  assert.equal(await page.locator('#return-position').isVisible(),false);
  assert.equal(await page.locator('#bookmark').getAttribute('aria-pressed'),'true');
  await page.locator('#contents').click();
  await page.locator('#contents-search').fill('zzzz-unmatched-fixture');
  assert.equal(await page.locator('#contents-empty').isVisible(),true);
  assert.equal(await page.locator('#contents-list button:visible').count(),0);
  const current=await page.evaluate(()=>stanza.state().page);
  await page.keyboard.press('PageDown');assert.equal(await page.evaluate(()=>stanza.state().page),current);
  await page.locator('#bookmarks-tab').click();
  assert.equal(await page.locator('.bookmark-row').count(),1);
  assert.equal(await page.locator('#contents-list').isVisible(),false);
  // Focus stays in the dialog; Escape works even in its input and restores the opener.
  await page.locator('.bookmark-row button').last().focus();await page.keyboard.press('Tab');
  assert.equal(await page.evaluate(()=>document.activeElement.id),'contents-done');
  await page.keyboard.press('Escape');assert.equal(await page.evaluate(()=>document.activeElement.id),'contents');
  await page.evaluate(()=>stanza.goToChapter(3));await settled();
  await page.locator('#contents').click();await page.locator('.bookmark-jump').click();await settled();
  assert.equal(await page.evaluate(()=>stanza.state().chapter),2);
  assert.equal(await page.evaluate(()=>document.activeElement===document.body),true);
  await page.locator('#return-position').click();await settled();assert.equal(await page.evaluate(()=>stanza.state().chapter),3);
  await page.locator('#contents').click();await page.locator('.bookmark-row button').last().click();
  assert.equal(await page.locator('#bookmarks-empty').isVisible(),true);
  await page.locator('#contents-tab').click();await page.locator('#contents-search').fill('');
  const target=await page.locator('#contents-list button').last().getAttribute('data-index');
  await page.locator('#contents-list button').last().click();await settled();
  assert.notEqual(await page.evaluate(()=>stanza.state().chapter),3);
  assert.ok(target);await page.locator('#return-position').click();await settled();assert.equal(await page.evaluate(()=>stanza.state().chapter),3);
  // Controls fit the narrow viewport and the reading footer never overflows horizontally.
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);
  await page.screenshot({path:`/tmp/stanza-affordances-${engine.name()}.png`});
 }finally{await browser.close();await new Promise(r=>server.close(r))}
});
