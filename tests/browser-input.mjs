// Start tests/catalog_server.py and pass the running web app URL as the first argument.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import path from 'node:path';
import {mkdtemp,rm} from 'node:fs/promises';
import os from 'node:os';
const {chromium,webkit}=createRequire(path.join(process.env.PLAYWRIGHT_ROOT,'anchor.cjs'))('playwright');
for(const engine of [chromium,webkit].filter(e=>!process.env.STANZA_BROWSER||e.name()===process.env.STANZA_BROWSER)){
 const profile=await mkdtemp(path.join(os.tmpdir(),'stanza-catalog-test-'));
 const context=await engine.launchPersistentContext(profile,{viewport:{width:1280,height:900}});
 try{
  const catalogUrl='http://127.0.0.1:18765/subjects?fixture='+Date.now();
  const page=context.pages()[0] || await context.newPage();await page.bringToFront();page.on('pageerror',e=>console.error(e.message));page.on('console',m=>{if(['error','warning'].includes(m.type()))console.error(m.text());});
  await page.goto(process.argv[2]);await page.locator('#navigation').waitFor();
  await page.evaluate(()=>location.hash='#catalogs');
  const input=id=>page.locator(`#${id} input, input#${id}`);
  await page.getByRole('button',{name:'Add OPDS catalog',exact:true}).click();
  await input('catalog-editor-url').fill(catalogUrl);
  await input('catalog-editor-title').fill('Fixture catalog');
  await page.locator('#catalog-editor-validate').click();await page.locator('#catalog-editor-valid').waitFor();
  assert.equal(await input('catalog-editor-title').inputValue(),'Fixture catalog');
  await page.locator('#catalog-editor-save').click();
  const list=t=>page.locator(`[id="catalog-list-${t}"]`);
  try {await list('1-Subjects').waitFor();} catch(e){console.error(await page.locator('body').innerText());throw e;}
  await list('1-Subjects').getByText('Continue browsing',{exact:true}).click();await list('2-Authors').waitFor();
  await page.locator('#catalog-browser').focus();await page.keyboard.press('ArrowLeft');await list('1-Subjects').waitFor();
  await page.keyboard.press('ArrowRight');await list('2-Authors').waitFor();
  const box=await list('2-Authors').boundingBox();await page.mouse.move(box.x+box.width/2,box.y+box.height/2);
  await page.mouse.wheel(-160,0);await list('1-Subjects').waitFor();await page.waitForTimeout(300);
  await page.mouse.wheel(160,0);await list('2-Authors').waitFor();
  await page.getByRole('button',{name:'Manage',exact:true}).click();
  await page.locator(`[id="catalog-edit-${catalogUrl}"]`).click();
  await input('catalog-editor-url').fill('http://127.0.0.1:18765/missing');
  await input('catalog-editor-url').focus();await page.keyboard.press('ArrowLeft');
  assert.equal(await input('catalog-editor-url').isVisible(),true);
  await page.locator('#catalog-editor-validate').click();await page.locator('#catalog-editor-error').waitFor();
  assert.equal(await page.locator('#catalog-editor-save').isEnabled(),false);
  await input('catalog-editor-url').fill(catalogUrl);
  await input('catalog-editor-title').fill('Persistent catalog');
  await page.locator('#catalog-editor-validate').click();await page.locator('#catalog-editor-valid').waitFor();
  await page.locator('#catalog-editor-save').click();
  await page.reload();await page.locator('#navigation').waitFor();await page.evaluate(()=>location.hash='#catalogs');
  await page.locator('#catalog-sources').getByText('Persistent catalog',{exact:true}).waitFor();
  await page.getByRole('button',{name:'Manage',exact:true}).click();
  await page.locator(`[id="catalog-remove-${catalogUrl}"]`).click();
  await page.screenshot({path:`/private/tmp/stanza-catalog-editor-${engine.name()}.png`});
  console.log(`${engine.name()}: PASS catalog validation/editing, keyboard, horizontal wheel, text-editing isolation and persistence`);
 }finally{await context.close();await rm(profile,{recursive:true,force:true});}
}
