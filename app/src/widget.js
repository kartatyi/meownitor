// The widget window: three modes — the cat alone, the card with the cat sitting on it, and a
// collapsed view at the left, right or top screen edge that slides the card out on hover. Dragging
// snaps to an edge (the top one has three magnets: left corner, centre, right corner); transparent pixels let clicks through; the cat's eyes follow the cursor anywhere on
// screen. Window geometry is Rust's, in physical pixels (main.rs) — this side only decides.
(function(){
  var invoke=window.__TAURI__.core.invoke,listen=window.__TAURI__.event.listen;
  var root=document.getElementById('root');
  var SNAP=24,BAND=58;
  // ax / ay: where the user put the content (see fit); anim: an animation owns the window (see animating).
  var S={mode:'cat',dock:null,mag:'c',open:false,idleOpen:false,back:false,ax:0,ay:0,anim:false,stale:false};
  var big=Pixel.Sprite('cat','catB',3,'idle'),small=Pixel.Sprite('cat','catB',2,'idle');
  // Settings live on the back of the card (config.json through Rust); SET holds what the back shows.
  var t=I18N.t;
  var KINDS={cat:['cat','catB'],blob:['blob','blob'],ghost:['ghost','ghost']};
  // How the widget looks collapsed at an edge: a strip of dots, the pet peeking out, or a thin line.
  var EDGES=['strip','peek','line'];
  var CFG={kind:'cat',sound:true,edge:'strip'},SET={autostart:false,hook:null,confirm:null,err:'',ver:''};
  // Only Windows has «start with Windows»: macOS and Linux start things at login (tauri-plugin-autostart's
  // LaunchAgent, an XDG autostart entry), and the switch says so.
  var OS=/Mac/.test(navigator.platform||navigator.userAgent)?'mac':/Linux/.test(navigator.platform||navigator.userAgent)?'linux':'';
  var minis={};Object.keys(KINDS).forEach(function(k){minis[k]=Pixel.Sprite(KINDS[k][0],KINDS[k][1],2,'idle');});
  var edgeMini=Pixel.Sprite('cat','catB',1,'idle');

  // Live data from Rust: the session list (sessions.rs) and the plan limits (limits.rs).
  var LIST=[],moodOverride=null,lastDone=0,LIM=null,LIMWHY='',LIMAT=0;
  var ICON={Bash:'ConsoleLine',PowerShell:'ConsoleLine',Edit:'PencilOutline',Write:'PencilOutline',MultiEdit:'PencilOutline',NotebookEdit:'PencilOutline',Read:'FileDocumentOutline',Grep:'Magnify',Glob:'Magnify',WebFetch:'Internet',WebSearch:'Internet',Task:'Magic',Agent:'Magic',Skill:'Magic','@agent':'Magic','@think':'Loading','@ask':'HelpCircleOutline','@perm':'KeyOutline','@done':'CheckCircleOutline','@failed':'AlertCircleOutline'};
  // The hook writes a tool's name or one of these codes (bin/hook.rs); a hook from before the
  // codes wrote the Ukrainian words themselves.
  var LEGACY={'Думає':'@think','Питає тебе':'@ask','Агент':'@agent','Чекає дозволу':'@perm','Готово — твоя черга':'@done','Зупинилась з помилкою':'@failed'};
  function whatOf(s){
    var w=s.what||'',x='',m=/^Чекає дозволу · (.+)$/.exec(w);
    if(m){w='@perm';x=m[1];}
    w=LEGACY[w]||w;
    return {code:w,text:(w.charAt(0)==='@'?t('what.'+w.slice(1)):w)+(x?' · '+x:'')+(s.detail?' · '+s.detail:'')};
  }
  // Rust leaves the title empty for a session Desktop does not know; it is named here, in the UI's language.
  function titleOf(s){return s.title||(s.cwd?t('terminal',{name:s.cwd.replace(/[\\\/]+$/,'').split(/[\\\/]/).pop()}):t('session',{id:s.sid.slice(0,8)}));}

  function esc(s){return String(s).replace(/[&<>"]/g,function(c){return {'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;'}[c];});}
  function ic(n){return '<svg class="i"><use href="#'+n+'"/></svg>';}
  function col(p){return p>=85?'var(--danger)':p>=60?'var(--warn)':'var(--primary)';}
  // How long the current state has lasted — or, for an idle session, how long ago it was active.
  function ago(since){
    var s=Math.max(0,Math.floor((Date.now()-since)/1000));
    if(s<60)return t('u.s',{n:s});var m=Math.floor(s/60);if(m<60)return t('u.min',{n:m});
    var h=Math.floor(m/60);if(h<24)return hm(h,m%60);return t('yesterday');
  }
  function hm(h,m){return t('u.h',{n:h})+(m?' '+t('u.min',{n:m}):'');}
  function until(ts){
    var m=Math.max(0,Math.round((ts-Date.now())/60000));
    if(m<60)return t('u.min',{n:m});var h=Math.floor(m/60);if(h<24)return hm(h,m%60);
    var d=Math.floor(h/24);return t('u.d',{n:d})+(h%24?' '+t('u.h',{n:h%24}):'');
  }
  // The usage response lists the limits the way Desktop's usage card shows them: `limits` —
  // session (5 hours), weekly_all, weekly_scoped per model — each with a percent and a reset time.
  // Older responses only had five_hour / seven_day objects with a utilization; those are the fallback.
  function normLimits(d){
    if(d&&Array.isArray(d.limits)&&d.limits.length)return d.limits.filter(function(x){return typeof x.percent==='number';}).map(function(x){
      var name=x.kind==='session'?t('lim.session'):x.kind==='weekly_all'?t('lim.week'):(x.scope&&x.scope.model&&x.scope.model.display_name)||x.kind;
      return {l:name,p:Math.round(x.percent),r:Date.parse(x.resets_at)};
    });
    var keys=Object.keys(d||{}).filter(function(k){return d[k]&&typeof d[k]==='object'&&typeof d[k].utilization==='number'&&/^(five_hour|seven_day)/.test(k);});
    keys.sort(function(a,b){var o=function(k){return k==='five_hour'?0:k==='seven_day'?1:2;};return o(a)-o(b)||a.localeCompare(b);});
    return keys.map(function(k){
      var v=d[k],r=v.resets_at,label=k==='five_hour'?t('lim.session'):k==='seven_day'?t('lim.week'):k.replace('seven_day_','');
      label=label.charAt(0).toUpperCase()+label.slice(1);
      return {l:label,p:Math.round(v.utilization),r:typeof r==='number'?(r<1e12?r*1000:r):Date.parse(r)};
    });
  }
  function maxLim(){return LIM&&LIM.length?LIM.reduce(function(a,b){return b.p>a.p?b:a;}):null;}
  function limHTML(){
    if(!LIM||!LIM.length)return '<div class="lim"><div class="nolim">'+t('lim.none',{why:LIMWHY==='net'||LIMWHY==='busy'||LIMWHY==='login'||LIMWHY==='stale'?t('lim.'+LIMWHY):'…'})+'</div></div>';
    // Through a hiccup (busy / net) the last numbers stay; once they are a few minutes old, say so.
    var stale=LIMWHY!=='ok'&&Date.now()-LIMAT>5*60000?'<div class="nolim st">'+t('lim.updated',{ago:'<span data-ago="'+LIMAT+'">'+agoTxt(LIMAT)+'</span>'})+'</div>':'';
    return '<div class="lim">'+LIM.map(function(l){return '<div class="lr"><span class="ll">'+esc(l.l)+'</span><span class="lp">'+l.p+'%</span><span class="lb"><i style="width:'+Math.min(100,l.p)+'%;background:'+col(l.p)+'"></i></span><span class="rs">'+ic('Autorenew')+'<span data-reset="'+l.r+'">'+until(l.r)+'</span></span></div>';}).join('')+stale+'</div>';
  }
  function agoTxt(ts){var a=ago(ts);return a===t('yesterday')?a:t('ago',{t:a});}
  function iconFor(s,code){return ICON[code]||(s.state==='wait'?'HelpCircleOutline':'ConsoleLine');}
  function row(s){
    var title=titleOf(s),w=whatOf(s);
    var h='<div class="row '+s.state+'" data-local="'+esc(s.local||'')+'"><span class="dot '+s.state+'"></span><div class="tx"><div class="l1"><span class="nm">'+esc(title)+'</span><span class="tm" data-since="'+s.since+'">'+ago(s.since)+'</span></div>';
    if(s.state!=='idle')h+='<div class="ac">'+ic(iconFor(s,w.code))+'<span class="at">'+esc(w.text)+'</span></div>';
    if(s.round)h+='<div class="qa"><button type="button" class="qbtn" data-act="round" data-sid="'+esc(s.sid)+'" data-round="'+esc(s.round)+'" data-title="'+esc(title)+'">'+ic('HelpCircleOutline')+t('answer')+'</button></div>';
    else if(s.state==='wait'&&s.local)h+='<div class="qa"><button type="button" class="qbtn" data-act="open">'+ic('HelpCircleOutline')+t('open.desktop')+'</button></div>';
    return h+'</div></div>';
  }
  function by(k){return LIST.filter(function(s){return s.state===k;});}
  function active(){return LIST.filter(function(s){return s.state!=='idle';});}
  function grp(title,arr){return arr.length?'<div class="gh"><span>'+title+'</span><span>'+arr.length+'</span></div>'+arr.map(row).join(''):'';}
  function card(edge){
    var idle=by('idle'),act=active();
    return '<div class="wg'+(edge?' e'+edge:'')+'"><div class="hdr grab">'+ic('ConsoleLine')+'<span class="tt">Claude</span><span class="ct">'+(act.length?t('active',{n:act.length}):t('quiet'))+'</span><span class="sp"></span><button type="button" class="ib" data-act="flip" title="'+t('settings')+'">'+ic('CogOutline')+'</button>'+(edge?'':'<button type="button" class="ib" data-act="min" title="'+t('minimize')+'">'+ic('Minus')+'</button>')+'</div>'+
      '<div class="ss">'+grp(t('g.wait'),by('wait'))+grp(t('g.run'),by('run'))+grp(t('g.done'),by('done'))+
      (idle.length?'<button type="button" class="grp'+(S.idleOpen?' open':'')+'" data-act="idle">'+ic('ChevronRight')+t('g.idle')+'<span class="n">'+idle.length+'</span></button>'+(S.idleOpen?idle.map(row).join(''):''):'')+
      (LIST.length?'':'<div class="empty">'+t('empty')+'</div>')+'</div>'+
      limHTML()+'</div>';
  }
  function hookHTML(){
    var h=SET.hook||{installed:false};
    if(SET.confirm==='install')return '<div class="hk"><div class="hk1">'+t('hook.ask.install')+'</div><div class="hkb"><button type="button" class="btn p" data-act="hook-yes">'+t('hook.add')+'</button><button type="button" class="btn" data-act="hook-no">'+t('no')+'</button></div></div>';
    if(SET.confirm==='uninstall')return '<div class="hk"><div class="hk1">'+t('hook.ask.uninstall')+'</div><div class="hkb"><button type="button" class="btn p" data-act="hook-yes">'+t('hook.remove')+'</button><button type="button" class="btn" data-act="hook-no">'+t('no')+'</button></div></div>';
    return '<div class="opt"><span>'+t('hook')+'<br><small class="'+(h.installed?'okc':'mut')+'">'+t(h.installed?'hook.on':'hook.off')+'</small></span><button type="button" class="btn" data-act="hook">'+t(h.installed?'hook.uninstall':'hook.install')+'</button></div>'+(SET.err?'<div class="err">'+esc(SET.err)+'</div>':'');
  }
  function settings(edge){
    return '<div class="wg'+(edge?' e'+edge:'')+'"><div class="hdr grab">'+ic('CogOutline')+'<span class="tt">'+t('settings')+'</span><span class="sp"></span><button type="button" class="ib" data-act="flip" title="'+t('back')+'">'+ic('Close')+'</button></div>'+
      '<div class="sec"><div class="sl">'+t('character')+'</div><div class="kinds">'+Object.keys(KINDS).map(function(k){return '<button type="button" class="kd'+(CFG.kind===k?' on':'')+'" data-act="kind" data-kind="'+k+'"><span class="mini" data-mini="'+k+'"></span><span>'+t('kind.'+k)+'</span></button>';}).join('')+'</div></div>'+
      '<div class="sec"><div class="sl">'+t('edge')+'</div><div class="kinds">'+EDGES.map(function(e){return '<button type="button" class="kd'+(CFG.edge===e?' on':'')+'" data-act="edge" data-edge="'+e+'">'+edgePreview(e)+'<span>'+t('edge.'+e)+'</span></button>';}).join('')+'</div></div>'+
      '<div class="opt"><span>'+t('language')+'</span><span class="seg">'+I18N.LANGS.map(function(l){return '<button type="button" class="'+(I18N.lang()===l[0]?'on':'')+'" data-act="lang" data-lang="'+l[0]+'">'+l[1]+'</button>';}).join('')+'</span></div>'+
      '<div class="opt"><span>'+t('sound')+'</span><button type="button" class="tgl'+(CFG.sound!==false?' on':'')+'" data-act="sound" title="'+t('sound.t')+'"></button></div>'+
      '<div class="opt"><span>'+t(OS?'autostart.'+OS:'autostart')+'</span><button type="button" class="tgl'+(SET.autostart?' on':'')+'" data-act="autostart" title="'+t('autostart.t')+'"></button></div>'+
      hookHTML()+'<div class="ver">'+esc(SET.ver)+'</div></div>';
  }
  // Collapsed at an edge. The strip: a dot per active session and the nearest limit as a bar. The pet
  // peeking out (half of it from a side, upside down from the top) over a short strip. The line: grey
  // when all is quiet, blue while something works; when a session waits for you it pulses and the
  // pet peeks out beside it. `.edge` itself is a transparent margin the «bump» moves into.
  function stripHTML(edge,cls){
    var m=maxLim(),act=active(),h=edge==='t';
    return '<div class="strip e'+edge+(cls||'')+'">'+(act.length?act.map(function(s){return '<span class="dot '+s.state+'"></span>';}).join(''):'<span class="dot"></span>')+
      (m?'<span class="vb'+(h?' h':'')+'"><i style="'+(h?'width':'height')+':'+Math.min(100,m.p)+'%;background:'+col(m.p)+'"></i></span>':'')+'</div>';
  }
  function edgeHTML(edge){
    var o='<div class="edge e'+edge+(performance.now()<bumpUntil?' bump':'')+'">';
    if(CFG.edge==='peek')return o+'<div class="pk"><div class="peek"></div>'+stripHTML(edge,' mini')+'</div></div>';
    // The pet's room beside the line is kept while it hides, so the window needn't grow when it comes out.
    if(CFG.edge==='line'){var st=by('wait').length?'wait':by('run').length?'run':'';return o+'<div class="lnw"><div class="peek'+(st==='wait'?'':' off')+'"></div><div class="lnh"><div class="ln '+st+'"></div></div></div></div>';}
    return o+stripHTML(edge)+'</div>';
  }
  var bumpUntil=0;
  function edgePreview(e){
    if(e==='peek')return '<span class="pv pk1" data-emini></span>';
    if(e==='line')return '<span class="pv ln1"></span>';
    return '<span class="pv st1"><i class="dot wait"></i><i class="dot run"></i><i class="dot done"></i></span>';
  }
  // The cat's mood follows the sessions: someone waiting for you beats everything, then work;
  // a session that just finished gets a little jump; long silence puts the cat to sleep.
  function autoMood(){
    if(by('wait').length)return 'ask';
    var m=maxLim();if(m&&m.p>=85)return 'tired';
    if(by('run').length)return 'work';
    if(Date.now()-lastDone<6000)return 'done';
    var newest=LIST.reduce(function(m,s){return Math.max(m,s.since);},0);
    return Date.now()-newest>20*60*1000?'sleep':'idle';
  }
  function setMood(m){if(big.mood!==m){big.mood=small.mood=m;big.t0=small.t0=performance.now();}}
  function applyMood(){setMood(moodOverride||autoMood());}

  function render(){
    if(S.dock&&!S.open)root.innerHTML=edgeHTML(S.dock);
    else if(S.mode==='cat'&&!S.dock)root.innerHTML='<div class="grab cat"></div>';
    else root.innerHTML='<div class="cardwrap'+(S.dock==='l'?' dl':'')+'"><div class="sit grab"></div>'+(S.back?settings(S.dock):card(S.dock))+'</div>';
    var c=root.querySelector('.cat');if(c)c.appendChild(big.canvas);
    var s=root.querySelector('.sit');if(s)s.appendChild(small.canvas);
    // The small pet also peeks out from an edge: mirrored at the left one, upside down (and without
    // its «?» / «z» bubbles, which would read upside down too) at the top one.
    var pk=root.querySelector('.peek:not(.off)');if(pk)pk.appendChild(small.canvas);
    small.flipX=!!pk&&S.dock==='l';small.flipY=small.nofx=!!pk&&S.dock==='t';
    var em=root.querySelector('[data-emini]');if(em)em.appendChild(edgeMini.canvas);
    root.querySelectorAll('[data-mini]').forEach(function(m){m.appendChild(minis[m.getAttribute('data-mini')].canvas);});
  }
  function save(patch){invoke('set_config',{patch:patch});}
  // Where the widget was left, so it comes back there: mode, docked edge, the content's top-left as
  // shown (x, y — it picks the monitor) and as the user put it (ax, ay — see fit).
  async function savePlace(){
    var p=await invoke('poll'),z=measure(p.scale);
    save({place:{mode:S.mode,dock:S.dock,mag:S.mag,x:Math.round(contentX(p,z)),y:Math.round(p.win.y),ax:Math.round(S.ax),ay:Math.round(S.ay)}});
  }
  function setKind(k){if(!KINDS[k])return;CFG.kind=k;big.kind=small.kind=edgeMini.kind=KINDS[k][0];big.pal=small.pal=edgeMini.pal=Pixel.PAL[KINDS[k][1]];}
  function measure(sc){var r=root.getBoundingClientRect();return {w:Math.ceil(r.width*sc),h:Math.ceil(r.height*sc)};}
  function clamp(v,a,b){return Math.max(a,Math.min(b,v));}

  // Windows keeps a captioned window at least SM_CXMIN (136 px) wide, so the window can be wider
  // than its content. The spare is transparent and click-through; at the right edge the content
  // hugs the window's right side, and the window always stays whole on its own monitor (a window
  // poking into a neighbouring monitor gets moved back by Windows).
  function align(){var r=S.dock==='r'||S.dock==='t'&&S.mag==='r';root.style.left=r?'auto':'0';root.style.right=r?'0':'auto';S.alignedR=r;}
  function contentX(p,z){return S.alignedR?p.win.x+p.win.w-z.w:p.win.x;}
  // Size the window to the content and place it: the content's top-left where the user put it
  // (S.ax, S.ay), or flush with the docked edge (at the top, on its magnet) — pushed back onto the
  // screen where it doesn't fit. S.ax / S.ay stay put meanwhile, so content that shrinks again (the
  // card folded back into the pet, the settings turned back to the sessions) comes back there rather
  // than staying where the bigger content was pushed. `moving`, if given, runs before the window moves.
  var WIDE={};
  async function fit(moving){
    var p=await invoke('poll'),sc=p.scale,z=measure(sc),k=p.work,key=sc+':'+z.w,aw=WIDE[key];
    if(aw==null){
      // The width the OS gives this content, learnt once: sized toward the side the content hugs, so
      // it shows nowhere else meanwhile.
      await invoke('place',{x:Math.round(S.alignedR?p.win.x+p.win.w-z.w:p.win.x),y:Math.round(p.win.y),w:z.w,h:z.h});
      p=await invoke('poll');aw=WIDE[key]=p.win.w;
    }
    var x=S.dock==='l'?k.x:S.dock==='r'?k.x+k.w-aw:S.dock==='t'?(S.mag==='l'?k.x:S.mag==='r'?k.x+k.w-aw:clamp(k.x+(k.w-z.w)/2,k.x,k.x+k.w-aw)):clamp(S.ax,k.x,k.x+k.w-aw);
    var y=S.dock==='t'?k.y:clamp(S.ay,k.y,k.y+k.h-z.h);
    x=Math.round(x);y=Math.round(y);
    if(x===p.win.x&&y===p.win.y&&aw===p.win.w&&z.h===p.win.h)return;
    if(moving&&(x!==p.win.x||y!==p.win.y))await moving();
    await invoke('place',{x:x,y:y,w:z.w,h:z.h});
  }
  // Re-render after a state change, then fit. 'toCard' / 'toCat': the card unfolds from the pet and
  // folds back into it, the pet keeping its middle (where the sitting pet has its middle too).
  // `hide(view)` holds the new view out of sight until the window is in place; `moving` goes to fit.
  async function relayout(how,hide,moving){
    var sc=(await invoke('poll')).scale,bz=measure(sc);
    render();align();
    var z=measure(sc);
    if(how==='toCard'){S.ax+=bz.w-z.w+4*sc;S.ay+=16*sc;}
    else if(how==='toCat'){S.ax+=bz.w-z.w-4*sc;S.ay-=16*sc;}
    if(hide)await hide(root.firstElementChild);
    await fit(moving);
  }

  // Motion, with the Web Animations API so each step can be awaited; none when the system asks for
  // less motion. A hidden page may never finish an animation or paint a frame, so neither is waited
  // on for longer than it should take.
  var EASE_OUT='cubic-bezier(.2,.8,.2,1)',EASE_IN='cubic-bezier(.5,0,.8,.3)';
  var LESS=window.matchMedia?matchMedia('(prefers-reduced-motion: reduce)'):{matches:false};
  function sleep(ms){return new Promise(function(r){setTimeout(r,ms);});}
  function play(el,frames,ms,ease,keep){
    if(!el||LESS.matches)return Promise.resolve();
    var a=el.animate(frames,{duration:ms,easing:ease,fill:keep?'forwards':'none'});
    return Promise.race([a.finished.catch(function(){}),sleep(ms+150)]);
  }
  // Until the screen shows what was just drawn. Wanted before the window moves while something shows:
  // for a frame or two the window carries the picture it had to its new place.
  function painted(){return Promise.race([new Promise(function(r){requestAnimationFrame(function(){requestAnimationFrame(r);});}),sleep(80)]);}
  // One animation at a time owns the window; data that comes meanwhile is drawn once it is done.
  async function animating(fn){
    if(S.anim)return;S.anim=true;
    try{await fn();}finally{S.anim=false;}
    if(S.stale)refresh();
  }
  // The view just rendered comes in from `from` (a keyframe of transform / opacity), held there while
  // the window is fitted; `part` animates a piece of it instead. `wait`: the old view still shows, so
  // the window moves only once it is gone.
  function enter(how,from,ms,part,wait){
    var to={};Object.keys(from).forEach(function(k){to[k]=k==='transform'?'none':k==='opacity'?1:from[k];});
    var pick=function(v){return part?v.querySelector(part):v;};
    return relayout(how,function(v){Object.assign(pick(v).style,from);return wait&&painted();}).then(function(){
      var el=pick(root.firstElementChild);Object.keys(from).forEach(function(k){el.style[k]='';});
      return play(el,[from,to],ms,EASE_OUT);
    });
  }
  // At an edge the view on screen slides into it and the next comes out of it: the card on hover, the
  // collapsed view once the cursor has left — and that one also after a drop at the edge.
  var OFF={r:'translateX(100%)',l:'translateX(-100%)',t:'translateY(-100%)'};
  function slide(open){return animating(async function(){
    await play(root.firstElementChild,[{transform:'none'},{transform:OFF[S.dock]}],open?90:160,EASE_IN,true);
    S.open=open;if(!open){S.back=false;SET.confirm=null;}
    await enter(null,{transform:OFF[S.dock]},open?220:180);
  });}
  function docked(){return animating(function(){return enter(null,{transform:OFF[S.dock]},200,null,true);});}
  // The pet opens its card: the card unfolds from where the pet sits, and folds back into it. The
  // window moves in between, while nothing shows.
  var FOLD='calc(100% - 58px) -26px';   // the sitting pet's middle, from the card's top-left
  function unfold(){return animating(async function(){
    S.mode='card';small.dropT=performance.now();
    await enter('toCard',{transformOrigin:FOLD,transform:'scale(.2)',opacity:0},240,'.wg',true);
  });}
  function fold(){return animating(async function(){
    await play(root.querySelector('.wg'),[{transformOrigin:FOLD,transform:'none',opacity:1},{transformOrigin:FOLD,transform:'scale(.2)',opacity:0}],180,EASE_IN,true);
    S.mode='cat';S.back=false;
    await relayout('toCat',function(v){v.style.opacity=0;return painted();});
    root.firstElementChild.style.opacity='';big.dropT=performance.now();
  });}
  // The card turns over: a quarter turn away, the other side drawn edge-on, a quarter turn back. It
  // shrinks a little on the way, so its near edge, coming closer, stays inside the window.
  function turn(a,s){return {transform:'perspective(1000px) rotateY('+a+'deg) scale('+s+')'};}
  function flip(){return animating(async function(){
    var back=!S.back,hop=false;
    // What the back shows loads while the card turns away.
    var load=back&&Promise.all([invoke('autostart_get').catch(function(){return SET.autostart;}),invoke('hook_status').catch(function(){return SET.hook;})]);
    await play(root.querySelector('.wg'),[turn(0,1),turn(90,.8)],150,EASE_IN,true);
    if(load){var got=await load;SET.autostart=got[0];SET.hook=got[1];}
    S.back=back;SET.confirm=null;SET.err='';
    // A taller side near the bottom of the screen moves the card up: the pet hops off meanwhile and
    // lands on the new top.
    await relayout(null,function(v){v.querySelector('.wg').style.transform=turn(-90,.8).transform;},function(){hop=true;root.querySelector('.sit').style.opacity=0;return painted();});
    var n=root.querySelector('.wg'),sit=root.querySelector('.sit');
    n.style.transform='';sit.style.opacity='';
    if(hop){small.dropT=performance.now();play(sit,[{transform:'translateY(-14px)',opacity:0},{transform:'none',opacity:1}],260,EASE_OUT);}
    await play(n,[turn(-90,.8),turn(0,1)],210,EASE_OUT);
  });}
  // At the line, the pet comes out from behind it when a session starts waiting.
  function peekOut(c){
    var b=c.parentNode.getBoundingClientRect(),d=S.dock,m=d==='l'?'scaleX(-1)':d==='t'?'scaleY(-1)':'';
    play(c,[{transform:(d==='l'?'translateX(-'+b.width+'px)':d==='t'?'translateY(-'+b.height+'px)':'translateX('+b.width+'px)')+' '+m},{transform:m||'none'}],320,EASE_OUT);
  }

  // Click-through: the window ignores the mouse unless the cursor is over something solid —
  // a card, the edge view or an opaque pixel of the pet. Polled, since an ignoring window gets no events.
  var ignoring=null,drag=null,lastInside=0,busy=false;
  function solidAt(x,y){
    var el=document.elementFromPoint(x,y);
    if(!el||el===document.body||el===document.documentElement||el===root||/(cardwrap|sit|cat|edge|pk|lnw|peek)/.test(el.getAttribute('class')||''))return false;
    if(el.tagName==='CANVAS'){var sp=el===big.canvas?big:small,r=el.getBoundingClientRect();return Pixel.opaqueAt(sp,x-r.left,y-r.top);}
    return true;
  }
  function setIgnore(on){if(on!==ignoring){ignoring=on;invoke('ignore',{on:on});}}
  async function poll(){
    if(busy)return;busy=true;
    try{
      var p=await invoke('poll'),sc=p.scale,x=(p.cursor[0]-p.win.x)/sc,y=(p.cursor[1]-p.win.y)/sc;
      Pixel.pointer.x=x;Pixel.pointer.y=y;
      var inside=x>=0&&y>=0&&x<p.win.w/sc&&y<p.win.h/sc,now=performance.now();
      if(!drag){
        var solid=inside&&solidAt(x,y);setIgnore(!solid);
        big.hover=small.hover=solid&&document.elementFromPoint(x,y)&&document.elementFromPoint(x,y).tagName==='CANVAS';
        if(big.hover)petAt(p.cursor[0]);else petX=null;
        if(S.dock&&!S.anim){
          if(!S.open&&solid){await slide(true);lastInside=performance.now();}
          else if(S.open&&!S.hold){if(inside)lastInside=now;else if(now-lastInside>450)await slide(false);}
        }
      }
    }finally{busy=false;}
  }
  setInterval(poll,70);
  // A hidden page's timers are throttled; the host's watchdog (watchdog.rs) mustn't take that for a hang.
  function visibility(){invoke('page_hidden',{hidden:document.hidden});}
  document.addEventListener('visibilitychange',visibility);visibility();

  // Dragging: by the cat, the card header or the strip. Rust keeps the grab offset and moves the
  // window to the cursor; on release we snap to an edge within SNAP px.
  var pending=false,pet=[],petX=null,petSign=0;
  // Petting: the cursor wiggled left and right over the pet. Fed from poll(), not pointermove: macOS
  // sends no mouse moves to a window that never becomes active, as ours never does.
  function petAt(sx){
    var now=performance.now();
    if(petX!=null&&Math.abs(sx-petX)<2)return;
    if(petX!=null){var sg=Math.sign(sx-petX);if(sg!==petSign){pet.push(now);petSign=sg;}}petX=sx;
    pet=pet.filter(function(t){return now-t<1200;});if(pet.length>=4){big.petUntil=small.petUntil=now+2200;pet=[];}
  }
  root.addEventListener('pointerdown',function(e){
    if(S.anim||e.button!==0||e.target.closest('button'))return;
    if(!e.target.closest('.grab,.edge'))return;
    drag={sx:e.screenX,sy:e.screenY,moved:false,onCat:!!e.target.closest('.cat')};
    try{root.setPointerCapture(e.pointerId);}catch(x){}
    e.preventDefault();
  });
  root.addEventListener('pointermove',async function(e){
    if(!drag)return;
    if(!drag.moved){
      if(Math.hypot(e.screenX-drag.sx,e.screenY-drag.sy)<4)return;
      drag.moved=true;
      if(S.dock){
        S.dock=null;S.open=false;render();align();
        var p=await invoke('poll'),sc=p.scale,z=measure(sc),gx=S.mode==='cat'?54:150,gy=S.mode==='cat'?40:BAND+17;
        await invoke('place',{x:Math.round(p.cursor[0]-gx*sc),y:Math.round(p.cursor[1]-gy*sc),w:z.w,h:z.h});
      }
      big.drag=small.drag=true;document.body.classList.add('dragging');
      await invoke('drag_start');
    }
    if(!pending){pending=true;requestAnimationFrame(function(){invoke('drag_move').finally(function(){pending=false;});});}
  });
  root.addEventListener('pointerup',async function(){
    if(!drag)return;var d=drag;drag=null;
    if(!d.moved){if(d.onCat&&S.mode==='cat'){await unfold();savePlace();}return;}
    big.drag=small.drag=false;big.dropT=small.dropT=performance.now();document.body.classList.remove('dragging');
    var p=await invoke('drag_end'),k=p.work,w=p.win,cw=measure(p.scale).w,edge=Math.round(SNAP*p.scale);
    // Where the user put it, as far as it fits on the screen (dragged, the content is at the window's left).
    S.ax=clamp(w.x,k.x,k.x+k.w-w.w);S.ay=clamp(w.y,k.y,k.y+k.h-w.h);
    // The nearest edge within SNAP px; at the top, the third of the screen the content's middle is in
    // picks the magnet.
    var near=[['l',w.x-k.x],['r',k.x+k.w-(w.x+cw)],['t',w.y-k.y]].filter(function(e){return e[1]<edge;}).sort(function(a,b){return a[1]-b[1];});
    S.dock=near.length?near[0][0]:null;
    if(S.dock==='t'){var mid=(w.x+cw/2-k.x)/k.w;S.mag=mid<1/3?'l':mid>2/3?'r':'c';}
    S.open=false;
    if(S.dock)await docked();else await relayout();
    savePlace();
  });
  root.addEventListener('click',async function(e){
    var a=e.target.closest('[data-act]'),r=e.target.closest('.row');
    if(!a){if(r&&r.dataset.local)invoke('open_session',{local:r.dataset.local});return;}
    if(S.anim)return;
    var act=a.getAttribute('data-act');
    if(act==='min'){await fold();savePlace();}
    else if(act==='idle'){S.idleOpen=!S.idleOpen;save({idleOpen:S.idleOpen});await relayout();}
    else if(act==='flip')await flip();
    else if(act==='kind'){setKind(a.dataset.kind);save({kind:CFG.kind});render();align();}
    else if(act==='edge'){CFG.edge=a.dataset.edge;save({edge:CFG.edge});render();align();}
    else if(act==='sound'){CFG.sound=CFG.sound===false;save({sound:CFG.sound});render();align();if(CFG.sound)chirp();}
    else if(act==='lang'){I18N.set(a.dataset.lang);await invoke('set_lang',{lang:I18N.lang()});await relayout();}
    else if(act==='autostart'){try{SET.autostart=await invoke('autostart_set',{on:!SET.autostart});SET.err='';}catch(x){SET.err=t('autostart.err',{e:x});}render();align();}
    else if(act==='hook'){SET.confirm=SET.hook&&SET.hook.installed?'uninstall':'install';await relayout();}
    else if(act==='hook-no'){SET.confirm=null;await relayout();}
    else if(act==='hook-yes'){var which=SET.confirm;SET.confirm=null;try{SET.hook=await invoke(which==='install'?'hook_install':'hook_uninstall');SET.err='';}catch(x){SET.err=String(x);}await relayout();}
    else if(act==='open'&&r&&r.dataset.local)invoke('open_session',{local:r.dataset.local});
    else if(act==='round')invoke('open_round',{sid:a.dataset.sid,name:a.dataset.round,title:a.dataset.title});
  });

  // New data: re-render what is on screen; resize the window only when the content's size changed,
  // so a session switching tools does not make the widget twitch.
  async function refresh(){
    if(!(S.dock||S.mode==='card')||S.back)return;
    var sc=(await invoke('poll')).scale;
    if(S.anim){S.stale=true;return;}
    S.stale=false;
    var bz=measure(sc),hid=!root.querySelector('.lnw .peek canvas');
    render();align();
    var z=measure(sc);if(z.w!==bz.w||z.h!==bz.h)await fit();
    var c=hid&&root.querySelector('.lnw .peek canvas');if(c)peekOut(c);
  }
  // The widget's only sound: a short chirp when a session starts waiting for you.
  var audio=null,primed=false;
  function chirp(){try{audio=audio||new AudioContext();[[660,0,.16],[880,.14,.24]].forEach(function(t){var o=audio.createOscillator(),g=audio.createGain(),at=audio.currentTime+t[1];o.frequency.setValueAtTime(t[0],at);g.gain.setValueAtTime(0,at);g.gain.linearRampToValueAtTime(.16,at+.02);g.gain.exponentialRampToValueAtTime(.001,at+t[2]);o.connect(g);g.connect(audio.destination);o.start(at);o.stop(at+t[2]+.05);});}catch(e){}}
  function onData(list){
    var wasDone={},wasWait={};LIST.forEach(function(x){if(x.state==='done')wasDone[x.sid]=1;if(x.state==='wait')wasWait[x.sid+'|'+(x.round||x.what)]=1;});
    if(list.some(function(x){return x.state==='done'&&!wasDone[x.sid];})&&LIST.length)lastDone=Date.now();
    var asks=primed&&list.some(function(x){return x.state==='wait'&&!wasWait[x.sid+'|'+(x.round||x.what)];});
    if(asks&&CFG.sound!==false)chirp();
    // Collapsed at an edge, the widget also nudges out once when a session starts waiting.
    if(asks&&S.dock&&!S.open){bumpUntil=performance.now()+800;setTimeout(function(){var e=root.querySelector('.edge.bump');if(e)e.classList.remove('bump');},800);}
    primed=true;
    LIST=list||[];applyMood();
    if(!drag)refresh();
  }
  listen('sessions',function(ev){onData(ev.payload);});
  function setLimits(l){l=l||{};LIM=l.data&&l.status!=='login'?normLimits(l.data):null;LIMWHY=l.status||'';LIMAT=l.at||0;applyMood();if(!drag)refresh();}
  listen('limits',function(ev){setLimits(ev.payload);});
  setInterval(function(){
    root.querySelectorAll('.tm[data-since]').forEach(function(t){t.textContent=ago(+t.getAttribute('data-since'));});
    root.querySelectorAll('[data-reset]').forEach(function(t){t.textContent=until(+t.getAttribute('data-reset'));});
    root.querySelectorAll('[data-ago]').forEach(function(t){t.textContent=agoTxt(+t.getAttribute('data-ago'));});
    applyMood();
  },1000);

  listen('tray',function(ev){
    var id=String(ev.payload);
    if(id.indexOf('mood:')===0){moodOverride=id==='mood:auto'?null:id.slice(5);applyMood();}
    else if(id.indexOf('kind:')===0){setKind(id.slice(5));save({kind:CFG.kind});if(S.back){render();align();}}
  });

  (async function start(){
    var hint=await invoke('start_hint'),how=hint[0]||'',cfg=await invoke('get_config')||{};
    I18N.set(cfg.lang);
    try{SET.ver=(await window.__TAURI__.app.getName())+' '+(await window.__TAURI__.app.getVersion());}catch(e){}
    if(cfg.kind)setKind(cfg.kind);
    if(cfg.sound===false)CFG.sound=false;
    if(EDGES.indexOf(cfg.edge)>=0)CFG.edge=cfg.edge;
    if(cfg.idleOpen)S.idleOpen=true;
    var place=!how&&cfg.place&&typeof cfg.place.x==='number'?cfg.place:null;
    if(place){S.mode=place.mode==='card'?'card':'cat';S.dock=/^[lrt]$/.test(place.dock)?place.dock:null;S.mag=/^[lcr]$/.test(place.mag)?place.mag:'c';}
    if(how==='card')S.mode='card';
    if(how==='settings'){S.mode='card';S.back=true;try{SET.autostart=await invoke('autostart_get');SET.hook=await invoke('hook_status');}catch(e){}}
    // dock-<edge>[-open][:<look>], e.g. dock-t-open or dock-r:peek.
    var look=how.split(':')[1];how=how.split(':')[0];if(EDGES.indexOf(look)>=0)CFG.edge=look;
    if(how.indexOf('dock-')===0){S.dock=how.charAt(5);S.open=/open$/.test(how);S.hold=S.open;}
    if(hint[1])moodOverride=hint[1];
    LIST=await invoke('sessions');setLimits(await invoke('limits'));applyMood();
    if(hint[2]){var rp=hint[2].split('/');invoke('open_round',{sid:rp[0],name:rp[1],title:t('check')});}
    render();align();
    var p=await invoke('poll'),sc=p.scale,z=measure(sc),k=p.work;
    // Back where it was left: move there first, so the clamp uses that monitor (or, if that monitor
    // is gone, the primary one).
    if(place){S.ax=typeof place.ax==='number'?place.ax:place.x;S.ay=typeof place.ay==='number'?place.ay:place.y;await invoke('place',{x:place.x,y:place.y,w:z.w,h:z.h});}
    else{S.ax=k.x+k.w-z.w-48*sc;S.ay=k.y+k.h-z.h-48*sc;}
    await fit();
    await invoke('show');
  })();
})();
