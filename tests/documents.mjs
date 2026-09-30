// Browser file input and the launchQueue intake. launchQueue is simulated; PWA OS registration
// itself needs an installed Chromium PWA and user approval.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {readFile} from 'node:fs/promises';
import path from 'node:path';
import {mkdtemp,rm} from 'node:fs/promises';
import os from 'node:os';
const {chromium,webkit}=createRequire(path.join(process.env.PLAYWRIGHT_ROOT,'anchor.cjs'))('playwright');
const bytes=await readFile('resource/assets/Alice.epub');
for(const engine of [chromium,webkit].filter(e=>!process.env.STANZA_BROWSER||e.name()===process.env.STANZA_BROWSER)){
 const profile=await mkdtemp(path.join(os.tmpdir(),'stanza-document-test-'));
 const context=await engine.launchPersistentContext(profile);
 try{
  await context.addInitScript(()=>{Object.defineProperty(window,'launchQueue',{value:{setConsumer:f=>window.documentConsumer=f},configurable:true});});
  const page=context.pages()[0] || await context.newPage();await page.bringToFront();await page.goto(process.argv[2]);await page.locator('#navigation').waitFor();
  const chooser=page.waitForEvent('filechooser');
  await page.getByRole('button',{name:'Open Book…',exact:true}).click();
  await (await chooser).setFiles({name:'Book with spaces.epub',mimeType:'application/epub+zip',buffer:bytes});
  const reader=()=>page.frameLocator('#book-webview');
  await reader().locator('#close-reader').waitFor();
  await page.waitForFunction(()=>document.querySelector('#book-webview')?.contentWindow.stanza?.state().ready);
  await reader().locator('#close-reader').click();await page.locator('#book-webview').waitFor({state:'detached'});
  await page.evaluate(async data=>window.documentConsumer({files:[{getFile:async()=>new File([Uint8Array.from(atob(data),c=>c.charCodeAt(0))],'Different filename.epub',{type:'application/epub+zip'})}]}),bytes.toString('base64'));
  await page.waitForFunction(()=>document.querySelector('#book-webview')?.contentWindow.stanza?.state().ready);
  assert.match(await page.evaluate(()=>document.querySelector('#book-webview').contentWindow.stanza.state().title),/Alice/);
  await reader().locator('#close-reader').click();await page.locator('#book-webview').waitFor({state:'detached'});
  console.log(`${engine.name()}: PASS file chooser and launchQueue delivery`);
 }finally{await context.close();await rm(profile,{recursive:true,force:true});}
}
