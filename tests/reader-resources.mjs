// Synthetic HTTP resource provider for browser layout tests. Native sanitization is tested in Rust.
export async function installResourceFixture(page) {
 await page.addInitScript(()=>{
  window.fixtureRequests=[];
  window.fixtureLoad=(data,options,position,translations)=>{
   const {assets,...manifest}=data;window.fixtureAssets=assets;window.fixtureRequests=[];
   return stanza.load(manifest,options,position,translations);
  };
  document.addEventListener('click',e=>{
   const a=e.target.closest?.('a');if(!a?.href.startsWith('stanza://'))return;
   e.preventDefault();e.stopImmediatePropagation();
  },true);
 });
 await page.route('**/book/**',async route=>{
  const path=decodeURIComponent(new URL(route.request().url()).pathname.split('/book/')[1]);
  const owner=route.request().frame().parentFrame() || page.mainFrame();
  const result=await owner.evaluate(path=>{
   window.fixtureRequests.push({path});return{asset:window.fixtureAssets?.[path],delay:window.fixtureDelay||0};
  },path);
  if(result.delay)await new Promise(r=>setTimeout(r,result.delay));
  if(!result.asset){await route.fulfill({status:404,body:''}).catch(()=>{});return;}
  let body=Buffer.from(result.asset.data,'base64');let mime=result.asset.mime;const headers={'Cache-Control':'no-store'};
  if(mime.includes('html')){
   body=await owner.evaluate(source=>{
    const doc=new DOMParser().parseFromString(source,'text/html');
    doc.querySelectorAll('script,iframe,object,embed,form,base,meta[http-equiv]').forEach(n=>n.remove());
    for(const el of doc.querySelectorAll('*'))for(const a of [...el.attributes])if(/^on/i.test(a.name)||a.name==='srcdoc')el.removeAttribute(a.name);
    return '<!doctype html>'+doc.documentElement.outerHTML;
   },body.toString());mime='text/html';
   headers['Content-Security-Policy']="default-src 'none'; script-src 'none'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; font-src 'self' data:; media-src 'self' data:; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'";
  }
  await route.fulfill({status:200,contentType:mime,headers,body}).catch(()=>{});
 });
}
