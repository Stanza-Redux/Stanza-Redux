// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
// Synthetic chapters deliberately end on both odd and even column counts.
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createServer} from 'node:http';
import {createRequire} from 'node:module';
import path from 'node:path';
import {installResourceFixture} from './reader-resources.mjs';
const {chromium,webkit}=createRequire(path.join(process.env.PLAYWRIGHT_ROOT||process.cwd(),'anchor.cjs'))('playwright');
const root=path.resolve('resource/assets/reader');
for(const engine of [chromium,webkit])test(`${engine.name()}: chapter boundary margins and complete spreads`,async()=>{
 const server=createServer(async(req,res)=>{try{const name=req.url==='/'?'/index.html':req.url;if(name.includes('..'))throw Error();res.setHeader('Content-Type',name.endsWith('.js')?'text/javascript':'text/html');res.end(await readFile(root+name))}catch{res.writeHead(404).end()}});
 await new Promise(r=>server.listen(0,'127.0.0.1',r));const browser=await engine.launch();
 try{
  const page=await browser.newPage({viewport:{width:1001,height:700}});await installResourceFixture(page);await page.goto(`http://127.0.0.1:${server.address().port}/`);
  const settled=()=>page.waitForFunction(()=>stanza.state().ready&&!stanza.state().animating);
  const check=async(margin,expectedPage,rtl)=>{
   const result=await page.evaluate(()=>{
    const f=document.getElementById('book'),d=f.contentDocument;
    return {state:stanza.state(),offset:f.contentWindow.scrollX,rects:[...d.querySelectorAll('p')].flatMap(p=>[...p.getClientRects()]).filter(r=>r.width>0&&r.right>1&&r.left<f.clientWidth-1).map(r=>({left:r.left,right:r.right}))};
   });
   assert.equal(result.state.page,expectedPage);
   assert.ok(Math.abs(result.offset-(rtl?-1:1)*expectedPage*result.state.width)<1,JSON.stringify(result));
   assert.ok(result.rects.length>0);
   for(const rect of result.rects){assert.ok(rect.left>=margin-1,JSON.stringify(result));assert.ok(rect.right<=result.state.width-margin+1,JSON.stringify(result));}
  };
  for(const rtl of [false,true])for(const columns of [1,2])for(const margin of [12,36])for(const count of [1,4,5]){
   const chapters=[{path:'one.xhtml',title:'Boundary fixture'},{path:'two.xhtml',title:'Next fixture'}];
   const html=n=>`<html dir="${rtl?'rtl':'ltr'}"><head></head><body>${Array.from({length:n},(_,i)=>`<p style="margin:0;${i?'break-before:column;':''}">Column ${i+1} synthetic text.</p>`).join('')}</body></html>`;
   const fixture={id:'margin-fixture',title:'Margin fixture',language:rtl?'ar':'en',rtl,chapters,toc:chapters,assets:Object.fromEntries(chapters.map((c,i)=>[c.path,{mime:'application/xhtml+xml',data:Buffer.from(html(i?1:count)).toString('base64')}]))};
   await page.evaluate(({fixture,columns,margin})=>fixtureLoad(fixture,{size:20,line:1.6,columns,margin},{},{locale:'en',position:'{page}/{pages}'}),{fixture,columns,margin});await settled();
   assert.equal(await page.evaluate(()=>stanza.state().pages),Math.ceil(count/columns));
   await check(margin,0,rtl);
   await page.locator('#chapter-progress').evaluate(el=>{el.value='1000';el.dispatchEvent(new Event('change'))});await settled();
   const last=Math.ceil(count/columns)-1;await check(margin,last,rtl);
   await page.evaluate(()=>stanza.next());await settled();assert.equal(await page.evaluate(()=>stanza.state().chapter),1);await check(margin,0,rtl);
   await page.evaluate(()=>stanza.previous());await settled();assert.equal(await page.evaluate(()=>stanza.state().chapter),0);await check(margin,last,rtl);
  }
 }finally{await browser.close();await new Promise(r=>server.close(r))}
});
