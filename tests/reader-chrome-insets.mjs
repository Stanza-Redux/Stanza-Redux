// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
// Synthetic text and safe-area padding reproduce the native iOS reader geometry.
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createServer} from 'node:http';
import {createRequire} from 'node:module';
import path from 'node:path';
import {installResourceFixture} from './reader-resources.mjs';
const {chromium,webkit}=createRequire(path.join(process.env.PLAYWRIGHT_ROOT||process.cwd(),'anchor.cjs'))('playwright');
const root=path.resolve('resource/assets/reader');
for(const engine of [chromium,webkit])test(`${engine.name()}: text stays clear of safe-area chrome across reflow`,async()=>{
 const server=createServer(async(req,res)=>{try{const name=req.url==='/'?'/index.html':req.url;if(name.includes('..'))throw Error();res.setHeader('Content-Type',name.endsWith('.js')?'text/javascript':'text/html');res.end(await readFile(root+name))}catch{res.writeHead(404).end()}});
 await new Promise(r=>server.listen(0,'127.0.0.1',r));const browser=await engine.launch();
 try{
  const page=await browser.newPage({viewport:{width:402,height:874}});await installResourceFixture(page);await page.goto(`http://127.0.0.1:${server.address().port}/`);
  await page.addStyleTag({content:'#toolbar{padding-top:67px!important}#footer{padding-bottom:37px!important}@media(max-height:450px){#toolbar{padding-top:5px!important}#footer{padding-bottom:24px!important}}'});
  const chapters=[{path:'one.xhtml',title:'Synthetic chapter'}];
  const html='<html><head></head><body>'+Array.from({length:60},(_,i)=>`<p>Paragraph ${i}: ${'Synthetic text for complete visible lines. '.repeat(12)}</p>`).join('')+'</body></html>';
  const fixture={id:'chrome-fixture',title:'Synthetic book',language:'en',chapters,toc:chapters,assets:{'one.xhtml':{mime:'application/xhtml+xml',data:Buffer.from(html).toString('base64')}}};
  await page.evaluate(fixture=>fixtureLoad(fixture,{size:20,line:1.6,margin:24,columns:1},{},{locale:'en',position:'{page}/{pages}',contents:'Contents fixture'}),fixture);
  const settled=()=>page.waitForFunction(()=>stanza.state().ready&&!stanza.state().animating);
  const check=async()=>{
   const bounds=await page.evaluate(()=>{
    const f=document.getElementById('book'),d=f.contentDocument,walker=d.createTreeWalker(d.body,NodeFilter.SHOW_TEXT),rects=[];
    while(walker.nextNode()){if(!walker.currentNode.textContent.trim())continue;const range=d.createRange();range.selectNodeContents(walker.currentNode);for(const r of range.getClientRects())if(r.width>0&&r.right>24&&r.left<f.clientWidth-24)rects.push({top:r.top,bottom:r.bottom});}
    return{rects,top:document.getElementById('toolbar').offsetHeight,bottom:innerHeight-document.getElementById('footer').offsetHeight};
   });
   assert.ok(bounds.rects.length,'The current page must contain visible text');
   for(const r of bounds.rects){assert.ok(r.top>=bounds.top+23,JSON.stringify({r,bounds}));assert.ok(r.bottom<=bounds.bottom-23,JSON.stringify({r,bounds}));}
  };
  await settled();await check();
  await page.evaluate(()=>stanza.next());await settled();await check();
  // Revealing controls must never change pagination or obscure newly visible lines.
  const before=await page.evaluate(()=>stanza.state());
  const toggle=()=>page.evaluate(()=>document.getElementById('book').contentDocument.body.dispatchEvent(new MouseEvent('click',{bubbles:true,clientX:innerWidth/2,clientY:innerHeight/2})));
  await toggle();await page.waitForTimeout(250);assert.equal(await page.evaluate(()=>stanza.state().controls),false);
  await toggle();await page.waitForTimeout(250);await check();
  assert.equal(await page.evaluate(()=>stanza.state().page),before.page);assert.equal(await page.evaluate(()=>stanza.state().pages),before.pages);
  // Footer wrapping / accessibility sizing can change without a window resize.
  const enlargedFooter=await page.addStyleTag({content:'#footer{padding-bottom:77px!important}'});
  await page.waitForTimeout(350);await settled();await check();
  await enlargedFooter.evaluate(el=>el.remove());
  for(const viewport of [{width:844,height:390},{width:320,height:568}]){
   await page.setViewportSize(viewport);await page.waitForTimeout(350);await settled();await check();
   await page.locator('#chapter-progress').evaluate(el=>{el.value='1000';el.dispatchEvent(new Event('change'))});await settled();await check();
  }
 }finally{await browser.close();await new Promise(r=>server.close(r))}
});
