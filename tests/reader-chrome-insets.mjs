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
for(const engine of [chromium,webkit])test(`${engine.name()}: text keeps its vertical margins across reflow`,async()=>{
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
  // Text sits between the vertical margins, each `vertical` of the page's height (no safe areas
  // in a desktop browser), and starts within a line of the top one.
  let vertical=0.15;
  const check=async()=>{
   const bounds=await page.evaluate(()=>{
    const f=document.getElementById('book'),d=f.contentDocument,walker=d.createTreeWalker(d.body,NodeFilter.SHOW_TEXT),rects=[],s=getComputedStyle(d.documentElement);
    while(walker.nextNode()){if(!walker.currentNode.textContent.trim())continue;const range=d.createRange();range.selectNodeContents(walker.currentNode);for(const r of range.getClientRects())if(r.width>0&&r.right>24&&r.left<f.clientWidth-24)rects.push({top:r.top,bottom:r.bottom});}
    return{rects,height:f.clientHeight,paddingTop:parseFloat(s.paddingTop),paddingBottom:parseFloat(s.paddingBottom)};
   });
   const inset=Math.round(vertical*bounds.height);
   assert.ok(Math.abs(bounds.paddingTop-inset)<=1&&Math.abs(bounds.paddingBottom-inset)<=1,JSON.stringify({vertical,inset,bounds}));
   assert.ok(bounds.rects.length,'The current page must contain visible text '+JSON.stringify({vertical,bounds}));
   for(const r of bounds.rects){assert.ok(r.top>=inset-1,JSON.stringify({r,inset}));assert.ok(r.bottom<=bounds.height-inset+1,JSON.stringify({r,inset}));}
   const first=Math.min(...bounds.rects.map(r=>r.top));assert.ok(first<inset+20*1.6,JSON.stringify({first,inset}));
  };
  await settled();await check();
  // The default clears the bars on a phone held upright, here with an iPhone's safe areas.
  const clears=await page.evaluate(()=>{const s=getComputedStyle(document.getElementById('book').contentDocument.documentElement);return parseFloat(s.paddingTop)>=document.getElementById('toolbar').offsetHeight&&parseFloat(s.paddingBottom)>=document.getElementById('footer').offsetHeight});
  assert.ok(clears,'The default vertical margin must clear the toolbar and footer');
  await page.evaluate(()=>stanza.next());await settled();await check();
  // Revealing controls must never change pagination.
  const before=await page.evaluate(()=>stanza.state());
  const toggle=()=>page.evaluate(()=>document.getElementById('book').contentDocument.body.dispatchEvent(new MouseEvent('click',{bubbles:true,clientX:innerWidth/2,clientY:innerHeight/2})));
  await toggle();await page.waitForTimeout(250);assert.equal(await page.evaluate(()=>stanza.state().controls),false);
  await toggle();await page.waitForTimeout(250);await check();
  assert.equal(await page.evaluate(()=>stanza.state().page),before.page);assert.equal(await page.evaluate(()=>stanza.state().pages),before.pages);
  // The setting's range: none, and a quarter above and below, leaving the text the middle half.
  for(vertical of [0,0.25,0.15]){
   await page.evaluate(vertical=>stanza.configure({size:20,line:1.6,margin:24,columns:1,vertical_margin:vertical}),vertical);await page.waitForTimeout(250);await settled();await check();
  }
  for(const viewport of [{width:844,height:390},{width:320,height:568}]){
   await page.setViewportSize(viewport);await page.waitForTimeout(350);await settled();await check();
   await page.locator('#chapter-progress').evaluate(el=>{el.value='1000';el.dispatchEvent(new Event('change'))});await settled();await check();
  }
 }finally{await browser.close();await new Promise(r=>server.close(r))}
});
