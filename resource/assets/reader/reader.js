// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
(()=>{
'use strict';
// The font setting's stacks, by `prefs.font`: three system stacks, then the bundled faces in
// fonts/ (see fonts/README.md). The chapter document lives under book/, so each face's URL is
// resolved against this page, and the private family names never match an installed font.
const FONTS=['Georgia,serif','system-ui,sans-serif','ui-monospace,monospace',"'Stanza Montserrat',system-ui,sans-serif","'Stanza Noto Sans',system-ui,sans-serif","'Stanza Noto Serif',Georgia,serif"];
const FONT_FACES=[['Stanza Montserrat','Montserrat-Regular.ttf','400'],['Stanza Noto Sans','NotoSans.ttf','100 900'],['Stanza Noto Serif','NotoSerif.ttf','100 900']].map(([family,file,weight])=>`@font-face{font-family:'${family}';src:url('${new URL('fonts/'+file,document.baseURI).href}') format('truetype');font-weight:${weight};font-style:normal;font-display:block}`).join('');
let frame=document.getElementById('book');let book=null,prefs={},chapter=0,page=0,pages=1,ready=false,serial=0;
function event(type,data={}){const a=document.getElementById('event');a.href='stanza://'+type+'?data='+encodeURIComponent(JSON.stringify(data));a.click()}
const resourceRoot=new URL('../book/',location.href);
function path(base,href){
 try {const url=new URL(href,new URL(base.split('/').map(encodeURIComponent).join('/'),resourceRoot));if(url.origin!==resourceRoot.origin||!url.pathname.startsWith(resourceRoot.pathname))return null;return decodeURIComponent(url.pathname.slice(resourceRoot.pathname.length))+url.hash;}catch{return null}
}
function releaseResources(el){el.onload=null;el.removeAttribute('srcdoc');el.src='about:blank'}
// Column overflow ends at the last occupied column, excluding the trailing margin
// (and an empty second column). Reserve the complete last spread before scrolling;
// otherwise scrollTo is clamped and the last page shifts toward the outer edge.
function reserveLastSpread(doc,width,count,rtl){
 const end=doc.createElement('div');end.setAttribute('aria-hidden','true');
 end.style.cssText=`position:absolute!important;left:${rtl?width-count*width:count*width-1}px!important;top:0!important;width:1px!important;height:1px!important;margin:0!important;padding:0!important;border:0!important;pointer-events:none!important;`;
 doc.documentElement.append(end);
}
function report(){positionChrome();event('position',{chapter,page,pages,title:book.chapters[chapter].title,progress:pages>1?page/(pages-1):0})}
function settings(){if(prefs.theme===3){const hex=n=>'#'+((n>>>0)&0xffffff).toString(16).padStart(6,'0');return{bg:hex(prefs.paper??0xf4ead6),fg:hex(prefs.ink??0x30343b)}}const dark=prefs.theme===2,sepia=prefs.theme===1;return {bg:dark?'#202326':sepia?'#f1e6cf':'#fafafa',fg:dark?'#e6e3dd':sepia?'#493f30':'#25272b'}}
// Keep keyboard focus in the stable shell before replacing a focused chapter.
// On web-dom, removing its iframe otherwise sends focus to the outer app document.
// Transfer synchronously and only from that chapter: an async load must never steal
// focus back from reader controls or another field in the surrounding app.
function releaseChapterFocus(el){
 if(document.hasFocus()&&document.activeElement===el)document.body.focus({preventScroll:true});
}
async function show(index,progress=0,fragment='',direction=0){
 if(!book)return;releaseChapterFocus(frame);cancelMotion();ready=false;document.getElementById('bookmark').disabled=true;document.getElementById('chapter-progress').disabled=true;
 let outgoing=null;
 if(direction){outgoing=frame;outgoing.id='book-outgoing';outgoing.style.pointerEvents='none';frame=outgoing.cloneNode(false);frame.removeAttribute('src');frame.id='book';frame.style.pointerEvents='';frame.hidden=true;outgoing.after(frame)}else releaseResources(frame)
 // Some Android WebViews retain a zero initial vh while their native view grows.
 // Read the current viewport and assign pixels before laying out the nested document.
 frame.style.height=Math.max(1,innerHeight)+'px';frame.style.width=Math.max(1,innerWidth)+'px';document.body.style.height=Math.max(1,innerHeight)+'px';
 chapter=Math.max(0,Math.min(index,book.chapters.length-1));const entry=book.chapters[chapter];
 const n=++serial,target=new URL(entry.path.split('/').map(encodeURIComponent).join('/'),resourceRoot).href;
 frame.onload=()=>{if(n!==serial)return;
 try {
 const doc=frame.contentDocument;if(!doc?.body||doc.URL!==target)return;
 doc.documentElement.lang=book.language||'';
 const colors=settings();document.documentElement.style.setProperty('--paper',colors.bg);document.documentElement.style.setProperty('--ink',colors.fg);document.body.style.background=colors.bg;
 const head=doc.head;
 const add=(content,first=false)=>{const el=doc.createElement('style');el.textContent=content;first?head.prepend(el):head.append(el)};
 add(FONT_FACES,true); add(window.stanzaCSS.before,true); if(!doc.querySelector('link[rel=stylesheet]'))add(window.stanzaCSS.default); add(window.stanzaCSS.after);
 const cols=prefs.columns===2&&frame.clientWidth>=640?2:1,gap=Math.max(12,prefs.margin||24),font=FONTS[prefs.font||0]||FONTS[0];
 // An explicit column width: iOS 18's WebKit lays out no columns at all for a column-count of 1
 // with an auto width, so a chapter ran down past the page instead of across pages.
 const colWidth=Math.max(1,Math.floor((frame.clientWidth-gap*2-(cols-1)*gap*2)/cols));
 doc.documentElement.style.cssText=`--USER__fontSize:${prefs.size||20}px;--USER__fontFamily:${font};--USER__fontOverride:readium-font-on;--USER__advancedSettings:readium-advanced-on;--USER__lineHeight:${prefs.line||1.6};--USER__colCount:${cols};--USER__textAlign:${prefs.justified?"justify":"start"};--USER__bodyHyphens:${prefs.hyphenation===false?"none":"auto"};--USER__backgroundColor:${colors.bg};--USER__textColor:${colors.fg};`;
 add(`html:root{height:${frame.clientHeight}px!important;min-height:${frame.clientHeight}px!important;max-height:${frame.clientHeight}px!important;width:${frame.clientWidth}px!important;min-width:${frame.clientWidth}px!important;max-width:${frame.clientWidth}px!important;box-sizing:border-box!important;padding:${gap+36}px ${gap}px ${gap+64}px!important;margin:0!important;column-count:${cols}!important;column-gap:${gap*2}px!important;column-width:${colWidth}px!important;column-fill:auto!important;overflow:hidden!important;background:${colors.bg}!important;color:${colors.fg}!important}body{margin:0!important;padding:0!important;max-width:none!important;min-width:0!important;font-family:${font}!important;font-size:${prefs.size||20}px!important;line-height:${prefs.line||1.6}!important;background:transparent!important;color:inherit!important}p{line-height:inherit!important;text-align:${prefs.justified?"justify":"start"}!important;hyphens:${prefs.hyphenation===false?"none":"auto"}!important;-webkit-hyphens:${prefs.hyphenation===false?"none":"auto"}!important;margin-block-end:${prefs.paragraph_spacing??0.6}em!important}img,svg{max-width:100%!important;max-height:${Math.max(1,frame.clientHeight-gap*2-100-(prefs.size||20)*(prefs.line||1.6))}px!important;object-fit:contain}a{color:inherit}*{animation:none!important;transition:none!important}`);
 const d=doc,w=frame.contentWindow;
 const layout=()=>{if(n!==serial)return;const width=frame.clientWidth;pages=Math.max(1,Math.ceil((d.documentElement.scrollWidth-1)/width));reserveLastSpread(d,width,pages,book.rtl);page=Math.max(0,Math.min(pages-1,Math.round(progress*(pages-1))));if(fragment){const el=d.getElementById(decodeURIComponent(fragment));if(el)page=Math.floor((el.getBoundingClientRect().left+Math.abs(w.scrollX))/width)}w.scrollTo({left:(book.rtl?-1:1)*page*width,behavior:'instant'});if(outgoing){slideChapters(outgoing,direction,report)}else{ready=true;report()}};
 gestures(d,entry);
 Promise.all([Promise.race([d.fonts?.ready,new Promise(r=>setTimeout(r,1500))]),...[...d.images].map(i=>i.complete?Promise.resolve():new Promise(r=>{i.onload=r;i.onerror=r;setTimeout(r,1500)}))]).then(()=>{let done=false;const once=()=>{if(!done){done=true;layout()}};requestAnimationFrame(once);setTimeout(once,100)});
 }catch(error){if(n===serial)event('error',{path:entry.path,message:error.message||''});}
 };frame.hidden=false;document.getElementById('hint').hidden=true;frame.src=target;
}
// WebKit requires sandbox allow-scripts even for parent-installed event listeners.
// EPUB scripts and event attributes are removed; the document CSP denies every script source.
// Reader chrome and gestures belong to the trusted shell, never the EPUB document.
let bookmarks=[], previousPosition=null, panelMode="contents";
let labels={}, controls=true, animating=false, motionToken=0, motionFrame=0, motionTimer=0;
const reduceMotion=()=>matchMedia('(prefers-reduced-motion: reduce)').matches;
function chrome(visible){
 controls=visible;document.body.classList.toggle('controls-hidden',!visible);
 for(const id of ['toolbar','footer']){const el=document.getElementById(id);el.inert=!visible;el.setAttribute('aria-hidden',String(!visible))}
 document.getElementById('gesture-hint').hidden=true;
}
function readerLabels(value={}){
 labels=value;
 document.documentElement.lang=labels.locale;
 document.getElementById("hint").textContent=labels.appTitle||"";
 document.getElementById("appearance").textContent=labels.appearanceGlyph||"";
 for(const [id,key] of [['close-reader','close'],['contents','contents'],['appearance','appearance'],['previous-page','previous'],['next-page','next']]){
  const el=document.getElementById(id);el.setAttribute('aria-label',labels[key]);el.title=labels[key];
 }
 document.getElementById('close-label').textContent=labels.close;
 document.getElementById('contents-title').textContent=labels.contents;
 document.getElementById('contents-done').textContent=labels.done;
 document.getElementById('gesture-hint').textContent=labels.hint;
 for(const [id,key] of [['contents-tab','contents'],['bookmarks-tab','bookmarks'],['contents-empty','contentsEmpty'],['bookmarks-empty','bookmarksEmpty']])document.getElementById(id).textContent=labels[key]||'';
 const back=document.getElementById('return-position');back.title=labels.returnPosition||'';back.setAttribute('aria-label',back.title);
 for(const [id,key] of [['contents-search','contentsSearch'],['chapter-progress','chapterPosition']])document.getElementById(id).setAttribute('aria-label',labels[key]||'');
 document.getElementById('contents-search').placeholder=labels.contentsSearch||'';
}
function renderContents(){
 const list=document.getElementById('contents-list');list.replaceChildren();
 book.toc.forEach((entry,index)=>{const button=document.createElement('button');button.textContent=entry.title;button.dataset.index=index;
  button.onclick=()=>{closeContents(true);window.stanza.jump(index)};list.append(button);
 });
 document.getElementById('book-title').textContent=book.title;
 frame.title=book.title;filterContents();
}
function locator(){return {chapter,progress:pages>1?page/(pages-1):0}}
function percent(value){return new Intl.NumberFormat(labels.locale,{style:'percent',maximumFractionDigits:0}).format(value)}
function chapterPercent(value){return (labels.chapterPercent||'').replace('{percent}',percent(value))}
function rememberPosition(){previousPosition=locator();document.getElementById('return-position').hidden=false}
function jumpPosition(target){if(!ready||animating)return;rememberPosition();show(target.chapter,target.progress)}
function currentBookmark(){return bookmarks.find(b=>b.chapter===chapter&&Math.round(b.progress*(pages-1))===page)}
function bookmarkChrome(){
 const button=document.getElementById('bookmark'),saved=currentBookmark();
 button.disabled=!ready;button.setAttribute('aria-pressed',String(Boolean(saved)));
 button.title=saved?labels.bookmarkRemove:labels.bookmarkAdd;button.setAttribute('aria-label',button.title||'');
}
function filterContents(){
 const needle=document.getElementById('contents-search').value.toLocaleLowerCase(labels.locale);
 let matches=0;document.querySelectorAll('#contents-list button').forEach(button=>{button.hidden=!button.textContent.toLocaleLowerCase(labels.locale).includes(needle);if(!button.hidden)matches++});
 document.getElementById('contents-empty').hidden=panelMode!=='contents'||matches>0;
}
function renderBookmarks(){
 const list=document.getElementById('bookmarks-list');list.replaceChildren();
 for(const mark of bookmarks){
  const row=document.createElement('div');row.className='bookmark-row';
  const jump=document.createElement('button');jump.className='bookmark-jump';
  const title=document.createElement('strong');title.textContent=mark.title;
  const subtitle=document.createElement('small');subtitle.textContent=chapterPercent(mark.progress);
  jump.append(title,subtitle);jump.onclick=()=>{closeContents(true);jumpPosition(mark)};
  const remove=document.createElement('button');remove.innerHTML='<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m7 7 10 10M17 7 7 17"/></svg>';remove.title=labels.bookmarkRemove||'';remove.setAttribute('aria-label',remove.title);
  remove.onclick=()=>event('bookmark',{remove:mark.id});row.append(jump,remove);list.append(row);
 }
 document.getElementById('bookmarks-empty').hidden=panelMode!=='bookmarks'||bookmarks.length>0;
 bookmarkChrome();
}
function contentsMode(mode){
 panelMode=mode;const contents=mode==='contents';
 document.getElementById('contents-tab').setAttribute('aria-pressed',String(contents));
 document.getElementById('bookmarks-tab').setAttribute('aria-pressed',String(!contents));
 document.getElementById('contents-search').hidden=!contents;document.getElementById('contents-list').hidden=!contents;
 document.getElementById('bookmarks-list').hidden=contents;filterContents();renderBookmarks();
}
function closeContents(reading=false){
 document.getElementById('contents-panel').hidden=true;frame.inert=false;
 for(const id of ['toolbar','footer'])document.getElementById(id).inert=!controls;
 (reading?document.body:document.getElementById('contents')).focus({preventScroll:true});
}
function positionChrome(){
 bookmarkChrome();
 const slider=document.getElementById('chapter-progress'),value=pages>1?page/(pages-1):0;
 slider.disabled=!ready||pages<=1;slider.value=Math.round(value*1000);slider.setAttribute('aria-valuetext',chapterPercent(value));
 document.getElementById('chapter-percent').textContent=percent(value);
 document.getElementById('reading-position').textContent=(labels.position||'').replace(/\{(chapter|chapters|page|pages)\}/g,(_,key)=>new Intl.NumberFormat(labels.locale).format({chapter:chapter+1,chapters:book.chapters.length,page:page+1,pages}[key]));
 document.getElementById('previous-page').disabled=chapter===0&&page===0;
 document.getElementById('next-page').disabled=chapter===book.chapters.length-1&&page===pages-1;
 document.querySelectorAll('#contents-list button').forEach(button=>button.setAttribute('aria-current',String(book.toc[Number(button.dataset.index)].path.split('#')[0]===book.chapters[chapter].path)));
}
function cancelMotion(){
 ++motionToken;cancelAnimationFrame(motionFrame);clearTimeout(motionTimer);animating=false;
 document.querySelectorAll('#book-outgoing').forEach(el=>{releaseChapterFocus(el);el.getAnimations().forEach(a=>a.cancel());releaseResources(el);el.remove()});
 frame.getAnimations().forEach(a=>a.cancel());frame.style.transform='';
}
function slideChapters(old,direction,done){
 const token=motionToken;animating=true;ready=false;
 const width=frame.clientWidth,sign=direction*(book.rtl?-1:1),duration=reduceMotion()?0:260;
 const outgoing=old.animate([{transform:'translateX(0)'},{transform:`translateX(${-sign*width}px)`}],{duration,easing:'cubic-bezier(.22,.7,.25,1)',fill:'forwards'});
 const incoming=frame.animate([{transform:`translateX(${sign*width}px)`},{transform:'translateX(0)'}],{duration,easing:'cubic-bezier(.22,.7,.25,1)',fill:'forwards'});
 let finished=false;const finish=()=>{if(finished||token!==motionToken)return;finished=true;clearTimeout(motionTimer);releaseChapterFocus(old);releaseResources(old);old.remove();incoming.cancel();outgoing.cancel();animating=false;ready=true;done()};
 incoming.finished.then(finish,()=>{});motionTimer=setTimeout(finish,duration+180);
}
function scrollPage(target,commit){
 cancelMotion();const token=motionToken,w=frame.contentWindow,start=w.scrollX,end=(book.rtl?-1:1)*target*frame.clientWidth;
 const duration=reduceMotion()?0:240,began=performance.now();animating=true;ready=false;
 let finished=false;const finish=()=>{if(finished||token!==motionToken)return;finished=true;cancelAnimationFrame(motionFrame);clearTimeout(motionTimer);w.scrollTo({left:end,behavior:'instant'});page=target;animating=false;ready=true;if(commit)report()};
 if(!duration||Math.abs(start-end)<1){finish();return}
 const step=now=>{if(token!==motionToken)return;const t=Math.min(1,(now-began)/duration),e=1-Math.pow(1-t,3);w.scrollTo({left:start+(end-start)*e,behavior:'instant'});if(t<1)motionFrame=requestAnimationFrame(step);else finish()};
 motionFrame=requestAnimationFrame(step);motionTimer=setTimeout(finish,duration+180);
}
function turn(delta){
 if(!book||!ready||animating)return false;
 const next=page+delta;
 if(next<0){if(chapter>0)show(chapter-1,1,'',-1);else scrollPage(page,false)}
 else if(next>=pages){if(chapter+1<book.chapters.length)show(chapter+1,0,'',1);else scrollPage(page,false)}
 else scrollPage(next,true);
 return true;
}
function key(e){
 if(e.defaultPrevented)return;
 const panel=document.getElementById('contents-panel');
 if(!panel.hidden){
  if(e.key==='Escape'){e.preventDefault();closeContents()}
  if(e.key==='Tab'){
   const targets=[...panel.querySelectorAll('button,input')].filter(el=>!el.hidden&&!el.disabled&&el.getClientRects().length);
   const first=targets[0],last=targets.at(-1);
   if(e.shiftKey&&document.activeElement===first){e.preventDefault();last?.focus()}
   else if(!e.shiftKey&&document.activeElement===last){e.preventDefault();first?.focus()}
  }
  return;
 }
 if(/INPUT|TEXTAREA|SELECT|BUTTON/.test(e.target.tagName)||e.target.isContentEditable||e.ctrlKey||e.metaKey||e.altKey)return;
 if(e.key==='Escape'){e.preventDefault();if(!controls){chrome(true)}else event('close');return}
 let delta=0;if(e.key==='ArrowRight')delta=book?.rtl?-1:1;if(e.key==='ArrowLeft')delta=book?.rtl?1:-1;
 if(e.key==='PageDown'||e.key===' ')delta=e.shiftKey?-1:1;if(e.key==='PageUp')delta=-1;
 if(delta){e.preventDefault();turn(delta)}
}

function gestures(d,entry){
 let pointer=null,suppressClickUntil=0;
 d.documentElement.style.touchAction='pan-y';
 const selected=()=>{const selection=d.getSelection();return Boolean(selection&&selection.rangeCount&&!selection.isCollapsed)};
 const restore=()=>{frame.style.transform='';scrollPage(page,false)};
 d.addEventListener('pointerdown',e=>{
  if(e.pointerType==='mouse'||e.isPrimary===false||!ready||animating)return;
  pointer={id:e.pointerId,x:e.clientX,y:e.clientY,time:performance.now(),scroll:frame.contentWindow.scrollX,drag:false};
 });
 d.addEventListener('pointermove',e=>{
  if(!pointer||pointer.id!==e.pointerId)return;
  const dx=e.clientX-pointer.x,dy=e.clientY-pointer.y;
  if(!pointer.drag&&Math.abs(dy)>Math.abs(dx)&&Math.abs(dy)>12){pointer=null;return}
  if(!pointer.drag&&Math.abs(dx)>10&&Math.abs(dx)>Math.abs(dy)&&!selected()){
   pointer.drag=true;try{d.documentElement.setPointerCapture(e.pointerId)}catch{}
  }
  if(pointer.drag){e.preventDefault();const logical=-dx*(book.rtl?-1:1),target=pointer.scroll-dx;
   if((logical>0&&page===pages-1)||(logical<0&&page===0)){frame.style.transform=`translateX(${dx*.3}px)`}
   else frame.contentWindow.scrollTo({left:target,behavior:'instant'});
  }
 },{passive:false});
 d.addEventListener('pointerup',e=>{
  if(!pointer||pointer.id!==e.pointerId)return;const p=pointer;pointer=null;
  if(!p.drag)return;
  suppressClickUntil=performance.now()+450;const dx=e.clientX-p.x,elapsed=performance.now()-p.time;
  if(Math.abs(dx)>45||(Math.abs(dx)>20&&elapsed<200)){
   frame.style.transform='';turn((dx<0?1:-1)*(book.rtl?-1:1));
  }else restore();
 });
 d.addEventListener('pointercancel',()=>{if(pointer?.drag){suppressClickUntil=performance.now()+450;restore()}pointer=null});
 d.addEventListener('click',e=>{
  if(performance.now()<suppressClickUntil){e.preventDefault();return}
  const a=e.target.closest?.('a[href]');
  if(a){e.preventDefault();const h=a.getAttribute('href'),p=path(entry.path,h);
   if(p){const [file,frag]=p.split('#'),i=book.chapters.findIndex(c=>c.path===file);if(i>=0){rememberPosition();show(i,0,frag||'')}}
   else if(/^https?:/.test(h))event('link',{url:h});return;
  }
  if(selected()||e.target.closest?.('button,input,textarea,select,audio,video'))return;
  const fraction=e.clientX/frame.clientWidth;
  // "Left tap advances" turns the left edge into a second forward zone for one-handed reading.
  if(fraction<.25)turn(prefs.left_tap_advances||book.rtl?1:-1);else if(fraction>.75)turn(book.rtl?-1:1);else chrome(!controls);
 });
 let wheelX=0,wheelY=0,wheelLast=0,wheelFired=false;
 d.addEventListener('wheel',e=>{
  const now=performance.now();if(now-wheelLast>240){wheelX=0;wheelY=0;wheelFired=false}wheelLast=now;
  if(e.ctrlKey||e.metaKey||e.target.closest?.('input,textarea,select'))return;
  const scale=e.deltaMode===1?16:e.deltaMode===2?frame.clientWidth:1;
  wheelX+=e.deltaX*scale;wheelY+=e.deltaY*scale;
  if(Math.abs(e.deltaX)>Math.abs(e.deltaY))e.preventDefault();
  if(!wheelFired&&Math.abs(wheelX)>70&&Math.abs(wheelX)>Math.abs(wheelY)*1.5){
   wheelFired=true;turn((wheelX>0?1:-1)*(book.rtl?-1:1));
  }
 },{passive:false});
 d.addEventListener('keydown',key);
}
document.addEventListener('keydown',key);
document.getElementById('close-reader').onclick=()=>{++serial;releaseResources(frame);cancelMotion();event('close')};
window.addEventListener('pagehide',()=>{++serial;releaseResources(frame);cancelMotion()});
document.getElementById('appearance').onclick=()=>event('settings');
document.getElementById('contents').onclick=()=>{
 document.getElementById('contents-panel').hidden=false;frame.inert=true;
 for(const id of ['toolbar','footer'])document.getElementById(id).inert=true;
 contentsMode(panelMode);document.getElementById('contents-done').focus();
};
document.getElementById('contents-done').onclick=()=>closeContents();
document.getElementById('contents-tab').onclick=()=>contentsMode('contents');
document.getElementById('bookmarks-tab').onclick=()=>contentsMode('bookmarks');
document.getElementById('contents-search').oninput=filterContents;
document.getElementById('bookmark').onclick=()=>{if(!ready||animating)return;const saved=currentBookmark();event('bookmark',saved?{remove:saved.id}:locator())};
document.getElementById('chapter-progress').oninput=e=>{const value=Number(e.target.value)/1000;document.getElementById('chapter-percent').textContent=percent(value);e.target.setAttribute('aria-valuetext',chapterPercent(value))};
document.getElementById('chapter-progress').onchange=e=>{if(!ready||animating)return;rememberPosition();scrollPage(Math.round(Number(e.target.value)/1000*(pages-1)),true)};
document.getElementById('return-position').onclick=()=>{if(!previousPosition||!ready||animating)return;const target=previousPosition;previousPosition=null;document.getElementById('return-position').hidden=true;document.body.focus({preventScroll:true});show(target.chapter,target.progress)};
document.getElementById('previous-page').onclick=()=>turn(-1);
document.getElementById('next-page').onclick=()=>turn(1);
window.stanza={load(data,options,position,translations,savedBookmarks=[]){++serial;cancelMotion();releaseResources(frame);readerLabels(translations);chrome(true);book=data;bookmarks=savedBookmarks;previousPosition=null;document.getElementById("return-position").hidden=true;renderBookmarks();prefs=options;renderContents();show(position?.chapter||0,position?.progress||0);return true},goToChapter(index){show(index)},next(){return turn(1)},previous(){return turn(-1)},configure(options){prefs=options;show(chapter,pages>1?page/(pages-1):0)},bookmarks(rows){bookmarks=rows;renderBookmarks()},jump(index){if(!ready||animating)return;const c=book.toc[index], [file,frag]=c.path.split('#');const i=book.chapters.findIndex(c=>c.path===file);if(i>=0){rememberPosition();show(i,0,frag||'')}},state(){return{ready,animating,controls,chapter,page,pages,width:frame.clientWidth,height:frame.clientHeight,title:book?.title,text:frame.contentDocument?.body?.innerText?.slice(0,100)}}};
let resizing;window.addEventListener('resize',()=>{clearTimeout(resizing);resizing=setTimeout(()=>{if(book)show(chapter,pages>1?page/(pages-1):0)},150)});
// The host's navigation delegate (or browser load hook) must be installed first.
let tries=0;const start=setInterval(()=>{if(book||++tries>50){clearInterval(start);return}event('ready')},150);
})();
