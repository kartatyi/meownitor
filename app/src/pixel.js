// Pixel-art sprite engine: the cat, the blob and the ghost, drawn part by part into a 36x32
// buffer (auto-outlined layers) and scaled up by whole pixels. Moods, idle life, blinking,
// dangling while dragged and petting are all frame parameters — see params().
(function(){
  var W=36,H=32;
  var PAL={
    catA:{id:'A',stripes:false,o:'#2F333B',f:'#B3B9C3',d:'#8C939F',b:'#F4F4F6',p:'#F2A7B5',n:'#E07F92',e:'#262A33',w:'#FFFFFF',k:'#F4B6C2',m:'#8E3A4C'},
    catB:{id:'B',stripes:true,o:'#202329',f:'#767E8C',d:'#4E5562',b:'#C3C8D0',p:'#E99AA8',n:'#D9788B',e:'#1C1F26',w:'#FFFFFF',k:'#E9A2B0',m:'#7A2E3E'},
    catC:{id:'C',stripes:false,o:'#283041',f:'#8D9CB5',d:'#6C7B95',b:'#AAB7CC',p:'#E6A2B2',n:'#D9849A',e:'#2F7A33',w:'#FFFFFF',k:'#E8AABA',m:'#7A2E3E'},
    blob:{id:'L',o:'#2C5F57',f:'#8FD9C8',d:'#62BBA7',b:'#C9F1E7',p:'#F28B82',n:'#F28B82',e:'#1F2D33',w:'#FFFFFF',k:'#F4A3A0',m:'#2F5B54',g:'#3F9C5E',G:'#7ACB8F'},
    ghost:{id:'G',o:'#5A6382',f:'#EEF1FB',d:'#CDD5EA',b:'#FFFFFF',p:'#F7A1B0',n:'#F7A1B0',e:'#2B2D42',w:'#FFFFFF',k:'#F7B3C0',m:'#2B2D42'}
  };
  var FXC={W:'#FFFFFF',K:'#3A3F4B',Q:'#E58A1E',C:'#B4BDCC',Y:'#FFD25E',B:'#6FC0F2',L:'#5C6475',S:'#3A4150',A:'#8C96A8',H:'#F26D7D'};
  var CACHE={};
  function rgb(pal,k){var key=pal.id+k;if(!CACHE[key]){var h=pal[k]||FXC[k]||'#FF00FF';CACHE[key]=[parseInt(h.substr(1,2),16),parseInt(h.substr(3,2),16),parseInt(h.substr(5,2),16)];}return CACHE[key];}
  function Buf(){var a=new Array(W*H);for(var i=0;i<a.length;i++)a[i]=null;return a;}
  function put(b,x,y,c){x=Math.round(x);y=Math.round(y);if(x>=0&&x<W&&y>=0&&y<H)b[y*W+x]=c;}
  function ell(b,cx,cy,rx,ry,c){for(var y=Math.floor(cy-ry-1);y<=Math.ceil(cy+ry+1);y++)for(var x=Math.floor(cx-rx-1);x<=Math.ceil(cx+rx+1);x++){var dx=(x+.5-cx)/rx,dy=(y+.5-cy)/ry;if(dx*dx+dy*dy<=1)put(b,x,y,c);}}
  function tri(b,ax,ay,bx,by,cx,cy,c){for(var y=Math.floor(Math.min(ay,by,cy));y<=Math.ceil(Math.max(ay,by,cy));y++)for(var x=Math.floor(Math.min(ax,bx,cx));x<=Math.ceil(Math.max(ax,bx,cx));x++){var px=x+.5,py=y+.5,d1=(px-bx)*(ay-by)-(ax-bx)*(py-by),d2=(px-cx)*(by-cy)-(bx-cx)*(py-cy),d3=(px-ax)*(cy-ay)-(cx-ax)*(py-ay);if(!((d1<0||d2<0||d3<0)&&(d1>0||d2>0||d3>0)))put(b,x,y,c);}}
  function rect(b,x,y,w,h,c){for(var j=0;j<h;j++)for(var i=0;i<w;i++)put(b,x+i,y+j,c);}
  function curve(b,x0,y0,x1,y1,x2,y2,r,c){for(var t=0;t<=1.001;t+=0.05){var u=1-t;ell(b,u*u*x0+2*u*t*x1+t*t*x2,u*u*y0+2*u*t*y1+t*t*y2,r,r,c);}}
  function outline(b){var o=b.slice();for(var y=0;y<H;y++)for(var x=0;x<W;x++){if(b[y*W+x])continue;if((x>0&&b[y*W+x-1])||(x<W-1&&b[y*W+x+1])||(y>0&&b[(y-1)*W+x])||(y<H-1&&b[(y+1)*W+x]))o[y*W+x]='o';}return o;}
  function comp(d,s){for(var i=0;i<d.length;i++)if(s[i])d[i]=s[i];}
  function layer(out,fn){var L=Buf();fn(L);comp(out,outline(L));}
  function glyph(b,rows,x,y){rows.forEach(function(r,j){for(var i=0;i<r.length;i++)if(r[i]!=='.')put(b,x+i,y+j,r[i]);});}

  function eye(L,cx,cy,p,side,tall){
    var x0=cx-1,lx=p.lx||0,ly=p.ly||0,t=p.eyes;
    if(t==='closed'){put(L,x0-1,cy,'e');put(L,x0,cy+1,'e');put(L,x0+1,cy+1,'e');put(L,x0+2,cy,'e');}
    else if(t==='happy'){put(L,x0-1,cy+1,'e');put(L,x0,cy,'e');put(L,x0+1,cy,'e');put(L,x0+2,cy+1,'e');}
    else if(t==='half'){rect(L,x0+lx,cy-1,2,1,'o');rect(L,x0+lx,cy,2,2,'e');}
    else if(t==='wide'||tall){rect(L,x0+lx,cy-2+ly,2,4,'e');put(L,x0+1+lx,cy-2+ly,'w');}
    else{rect(L,x0+lx,cy-1+ly,2,3,'e');put(L,x0+1+lx,cy-1+ly,'w');}
  }
  function eyes(L,x1,x2,y,p,tall){eye(L,x1,y,p,-1,tall);eye(L,x2,y,p,1,tall);}
  function mouth(L,y,m){
    if(m==='open'){rect(L,17,y,2,2,'m');}
    else if(m==='yawn'){rect(L,16,y,4,2,'m');rect(L,17,y+2,2,1,'m');}
    else if(m==='smile'){put(L,16,y,'o');put(L,19,y,'o');rect(L,17,y,2,2,'m');}
    else if(m==='none'){put(L,17,y+1,'o');put(L,18,y+1,'o');}
    else{put(L,16,y,'o');put(L,17,y+1,'o');put(L,18,y+1,'o');put(L,19,y,'o');}
  }
  function laptop(out,Y,paws,col){
    layer(out,function(L){rect(L,10,21+Y,16,7,'L');put(L,17,24+Y,'A');put(L,18,24+Y,'A');rect(L,7,28+Y,22,2,'S');});
    layer(out,function(L){ell(L,14,28.2+Y-(paws==='type0'?1:0),1.9,1.3,col);ell(L,22,28.2+Y-(paws==='type1'?1:0),1.9,1.3,col);});
  }
  function waveArm(out,Y,frame,bx,by,col,ends){var e=ends[frame%ends.length];layer(out,function(L){curve(L,bx,by+Y,(bx+e[0])/2+1,(by+e[1])/2+Y,e[0],e[1]+Y,1.2,col);ell(L,e[0],e[1]+Y-.5,2,1.8,col);});}

  function drawCat(p,pal){
    var out=Buf(),Y=p.y||0,s=p.tail||0;
    layer(out,function(L){if(p.dangle){curve(L,23,25+Y,26,28+Y,25+s,31+Y,1.1,'f');}else{curve(L,24,27+Y,30+s*.4,27+Y,30+s,19+Y,1.15,'f');ell(L,30+s,19.2+Y,1.15,1.15,'d');}});
    layer(out,function(L){if(p.dangle){ell(L,18,22.5+Y,6.5,7,'f');ell(L,18,24+Y,3.5,4.5,'b');}else{ell(L,18,24.5+Y,8,6,'f');ell(L,18,25.5+Y,4.5,4,'b');if(pal.stripes){put(L,11,23+Y,'d');put(L,11,24+Y,'d');put(L,24,23+Y,'d');put(L,24,24+Y,'d');put(L,12,26+Y,'d');put(L,23,26+Y,'d');}}});
    if(p.dangle)layer(out,function(L){ell(L,14.5,29.5+Y,1.6,1.8,'f');ell(L,21.5,29.5+Y,1.6,1.8,'f');});
    else if(!p.laptop)layer(out,function(L){ell(L,14.5,29.6+Y,2.3,1.5,'f');if(p.wave==null)ell(L,21.5,29.6+Y,2.3,1.5,'f');});
    if(p.laptop)laptop(out,Y,p.paws,'f');
    var hy=13+Y+(p.hy||0),el=p.earL||0,er=p.earR||0;
    var H1=Buf();
    tri(H1,9.5,hy-3,10.5-el*1.2,hy-11+el*2,15,hy-6,'f');tri(H1,26.5,hy-3,25.5+er*1.2,hy-11+er*2,21,hy-6,'f');
    ell(H1,18,hy,9,7.5,'f');
    tri(H1,11,hy-4.5,11.6-el,hy-8.5+el*1.6,13.8,hy-6,'p');tri(H1,25,hy-4.5,24.4+er,hy-8.5+er*1.6,22.2,hy-6,'p');
    put(H1,18,hy-6,'d');put(H1,18,hy-5,'d');put(H1,15,hy-5,'d');put(H1,21,hy-5,'d');if(pal.stripes){put(H1,14,hy-4,'d');put(H1,22,hy-4,'d');put(H1,10,hy,'d');put(H1,26,hy,'d');}
    ell(H1,18,hy+4.2,3.4,2.4,'b');
    var O=outline(H1);
    eyes(O,14,23,hy,p);
    put(O,17,hy+3,'n');put(O,18,hy+3,'n');
    mouth(O,hy+4,p.mouth);
    put(O,11,hy+3,'k');put(O,25,hy+3,'k');
    comp(out,O);
    put(out,6,hy+3,'d');put(out,7,hy+3,'d');put(out,29,hy+3,'d');put(out,30,hy+3,'d');put(out,7,hy+5,'d');put(out,29,hy+5,'d');
    if(p.wave!=null)waveArm(out,Y,p.wave,24,25,'f',[[29,15],[30,13],[28,14],[30,13]]);
    return out;
  }
  function drawBlob(p,pal){
    var out=Buf(),Y=p.y||0,sq=p.dangle?0:(p.hy?0.7:0),rx=11+sq,ry=9-sq,cy=30-ry+Y;
    if(p.dangle){rx=8;ry=11;cy=19+Y;}
    if(p.squash){rx+=1.5;ry-=1.5;cy=30-ry+Y;}
    var ty=Math.round(cy-ry),sw=(p.tail||0)>0?1:(p.tail||0)<0?-1:0;
    layer(out,function(L){put(L,18,ty,'g');put(L,18,ty-1,'g');put(L,17+sw,ty-2,'g');put(L,16+sw,ty-2,'G');put(L,15+sw,ty-3,'G');put(L,19+sw,ty-2,'g');put(L,20+sw,ty-2,'G');put(L,21+sw,ty-3,'G');});
    layer(out,function(L){ell(L,6.8,cy+2,1.6,2.3,'d');if(p.wave==null&&!p.laptop)ell(L,29.2,cy+2,1.6,2.3,'d');});
    var B=Buf();ell(B,18,cy,rx,ry,'d');ell(B,17.6,cy-.8,rx-.5,ry-.8,'f');put(B,12,Math.round(cy-4),'b');put(B,13,Math.round(cy-5),'b');put(B,14,Math.round(cy-5),'b');
    var O=outline(B),ey=Math.round(cy-1);
    eyes(O,14,23,ey,p);put(O,10,ey+3,'k');put(O,26,ey+3,'k');mouth(O,ey+3,p.mouth);
    comp(out,O);
    if(p.laptop)laptop(out,Y,p.paws,'d');
    if(p.wave!=null){var e=[[30,ey-4],[31,ey-6],[30,ey-5],[31,ey-6]][p.wave%4];layer(out,function(L){ell(L,e[0],e[1],1.7,2.3,'d');});}
    return out;
  }
  function drawGhost(p,pal){
    var out=Buf(),fy=(p.y||0)+(p.dangle?0:p.fl||0),wp=p.wp||0;
    var G=Buf();ell(G,18,12+fy,10,9.5,'f');
    for(var x=8;x<=28;x++){var bot=25+Math.round(1.2*Math.sin((x+wp)*0.9))+fy;for(var y=12+fy;y<=bot;y++)put(G,x,y,'f');put(G,x,bot,'d');}
    put(G,12,6+fy,'b');put(G,13,5+fy,'b');put(G,14,5+fy,'b');
    var O=outline(G),ey=13+fy;
    eyes(O,14,23,ey,p,true);put(O,11,ey+4,'k');put(O,26,ey+4,'k');mouth(O,ey+4,p.mouth);
    comp(out,O);
    layer(out,function(L){ell(L,6.5,18+fy,1.5,2.2,'f');if(p.wave==null&&!p.laptop)ell(L,29.5,18+fy,1.5,2.2,'f');});
    if(p.laptop)laptop(out,0,p.paws,'f');
    if(p.wave!=null){var e=[[30,12],[31,10],[30,11],[31,10]][p.wave%4];layer(out,function(L){ell(L,e[0],e[1]+fy,1.5,2.2,'f');});}
    return out;
  }
  function fx(b,p){
    var k=p.ph||0;
    if(p.fx==='q'){var y0=1-(k%2);rect(b,26,y0,9,8,'K');rect(b,27,y0+1,7,6,'W');put(b,27,y0+8,'K');put(b,28,y0+8,'K');glyph(b,['QQQ','..Q','.QQ','...','.Q.'],29,y0+1);}
    else if(p.fx==='dots'){rect(b,26,1,9,6,'K');rect(b,27,2,7,4,'W');put(b,27,7,'K');put(b,28,7,'K');for(var i=0;i<3;i++)put(b,28+i*2,3,i===k%3?'K':'C');}
    else if(p.fx==='z'){var q=k%8;glyph(b,['CCCC','..C.','.C..','CCCC'],26+(q>>1),9-q);if(q<5)glyph(b,['CCC','.C.','CCC'],31,6-q);}
    else if(p.fx==='spark'){var on=k%2;[[3,6,on],[31,9,1-on],[5,19,1-on],[30,22,on]].forEach(function(s){if(s[2]){put(b,s[0],s[1],'Y');put(b,s[0]-1,s[1],'Y');put(b,s[0]+1,s[1],'Y');put(b,s[0],s[1]-1,'Y');put(b,s[0],s[1]+1,'Y');}else put(b,s[0],s[1],'Y');});}
    else if(p.fx==='sweat'){var s=k%7;if(s<6){put(b,28,4+s,'B');put(b,28,5+s,'B');put(b,27,5+s,'B');put(b,29,5+s,'B');put(b,28,6+s,'B');}}
    else if(p.fx==='heart'){var h=k%10;glyph(b,['.H.H.','HHHHH','.HHH.','..H..'],27,9-h);if(h>3)glyph(b,['.H.H.','HHHHH','.HHH.','..H..'],4,13-h);}
  }

  var SPR=[];
  function Sprite(kind,pal,scale,mood){
    var c=document.createElement('canvas');c.className='spr';c.width=W*scale;c.height=H*scale;
    var off=document.createElement('canvas');off.width=W;off.height=H;
    var sp={kind:kind,pal:PAL[pal],scale:scale,mood:mood||'idle',canvas:c,ctx:c.getContext('2d'),off:off,octx:off.getContext('2d'),t0:performance.now()-Math.random()*3000,nextBlink:0,nextLife:performance.now()+2000+Math.random()*5000,life:null,lookX:0,lookY:0,hover:false,drag:false,petUntil:0,dropT:0};
    sp.img=sp.octx.createImageData(W,H);
    SPR.push(sp);return sp;
  }
  function params(sp,now){
    var t=now-sp.t0,m=sp.mood,p={y:0,hy:0,eyes:'open',lx:0,ly:0,mouth:'idle',tail:0,wave:null,earL:0,earR:0,paws:null,laptop:false,dangle:false,fx:null,ph:0},look=false;
    p.fl=Math.round(1.3*Math.sin(t/480));p.wp=Math.floor(t/110);
    if(sp.drag){p.dangle=true;p.eyes='wide';p.mouth='open';p.ly=-1;p.tail=Math.round(1.5*Math.sin(t/110));p.wp=Math.floor(t/50);return p;}
    if(sp.petUntil>now){p.eyes='happy';p.mouth='smile';p.earL=p.earR=1;p.hy=Math.floor(t/220)%2;p.tail=Math.round(2.5*Math.sin(t/140));p.fx='heart';p.ph=Math.floor(t/110);return p;}
    if(m==='work'){p.laptop=true;p.ly=1;var cyc=t%3400;p.paws=cyc>2600?'rest':(Math.floor(t/130)%2?'type0':'type1');p.tail=Math.round(Math.sin(t/600));p.fx='dots';p.ph=Math.floor(t/320);p.hy=cyc>2600?0:Math.floor(t/520)%2;}
    else if(m==='ask'){p.eyes='wide';p.mouth='open';p.wave=Math.floor(t/160);p.fx='q';p.ph=Math.floor(t/380);p.hy=Math.floor(t/380)%2;p.tail=Math.round(2*Math.sin(t/240));look=true;}
    else if(m==='done'){var seq=[0,1,1,-2,-4,-5,-5,-4,-2,0,1,0,0,0,0,0,0,0];var i=Math.floor(t/70)%seq.length;p.y=seq[i];p.squash=seq[i]===1;p.eyes='happy';p.mouth='smile';p.fx='spark';p.ph=Math.floor(t/210);p.tail=Math.round(3*Math.sin(t/150));}
    else if(m==='sleep'){p.eyes='closed';p.mouth='none';p.hy=Math.floor(t/1300)%2;p.earL=p.earR=.7;p.fx='z';p.ph=Math.floor(t/330);p.tail=0;p.fl=Math.round(Math.sin(t/900));p.wp=Math.floor(t/300);}
    else if(m==='tired'){p.eyes='half';p.earL=p.earR=1.7;p.hy=Math.floor(t/950)%2;p.fx='sweat';p.ph=Math.floor(t/190);p.tail=Math.round(Math.sin(t/950));look=true;p.wp=Math.floor(t/220);}
    else{
      p.hy=Math.floor(t/700)%2;p.tail=Math.round(2*Math.sin(t/(sp.hover?170:430)));look=true;
      if(!sp.life&&now>sp.nextLife){var acts=[['look',2200],['yawn',1400],['ear',520],['flick',800],['look',2200]];var a=acts[Math.floor(Math.random()*acts.length)];sp.life={type:a[0],start:now,until:now+a[1]};}
      if(sp.life){var L=sp.life,lt=now-L.start;if(now>L.until){sp.life=null;sp.nextLife=now+5000+Math.random()*7000;}
        else if(L.type==='look'){p.lx=lt<1000?-1:1;p.ly=0;look=false;}
        else if(L.type==='yawn'){p.eyes='closed';p.mouth=lt>200&&lt<1150?'yawn':'idle';p.earL=p.earR=.6;p.hy=1;}
        else if(L.type==='ear'){p.earR=Math.floor(lt/90)%2?1.2:0;}
        else if(L.type==='flick'){p.tail=Math.round(3*Math.sin(lt/70));}}
    }
    if(sp.dropT&&now-sp.dropT<500){var ds=[1,1,-2,-3,-2,0,1,0];p.y+=ds[Math.min(ds.length-1,Math.floor((now-sp.dropT)/60))];}
    if(look){p.lx=sp.lookX;if(p.eyes!=='half')p.ly=sp.lookY;}
    if((p.eyes==='open'||p.eyes==='wide'||p.eyes==='half')&&now>sp.nextBlink){if(now<sp.nextBlink+130)p.eyes='closed';else sp.nextBlink=now+2400+Math.random()*3200;}
    return p;
  }
  function draw(sp,now){
    var p=params(sp,now),b=sp.kind==='blob'?drawBlob(p,sp.pal):sp.kind==='ghost'?drawGhost(p,sp.pal):drawCat(p,sp.pal);if(!sp.nofx)fx(b,p);sp.buf=b;
    var d=sp.img.data;for(var i=0;i<W*H;i++){var k=b[i];if(!k){d[i*4+3]=0;continue;}var c=rgb(sp.pal,k);d[i*4]=c[0];d[i*4+1]=c[1];d[i*4+2]=c[2];d[i*4+3]=255;}
    sp.octx.putImageData(sp.img,0,0);sp.ctx.imageSmoothingEnabled=false;sp.ctx.clearRect(0,0,sp.canvas.width,sp.canvas.height);sp.ctx.drawImage(sp.off,0,0,sp.canvas.width,sp.canvas.height);
  }
  var PTR={x:-1e5,y:-1e5};
  function tick(){
    var now=performance.now();
    SPR.forEach(function(sp){
      if(!sp.canvas.isConnected)return;
      var r=sp.canvas.getBoundingClientRect();if(!r.width)return;
      var dx=PTR.x-(r.left+r.width/2),dy=PTR.y-(r.top+r.height*0.4);
      sp.lookX=dx>r.width*0.45?1:dx<-r.width*0.45?-1:0;sp.lookY=dy>r.height*0.5?1:dy<-r.height*0.5?-1:0;
      // A mirrored sprite (peeking from the left or the top edge) looks the other way in its own pixels.
      if(sp.flipX)sp.lookX=-sp.lookX;if(sp.flipY)sp.lookY=-sp.lookY;
      draw(sp,now);
    });
  }
  setInterval(tick,70);
  // Is the pixel under (x,y) — in the canvas's own CSS pixels — part of the sprite? A canvas drawn
  // mirrored (flipX / flipY, a CSS flip) is looked up mirrored.
  function opaqueAt(sp,x,y){if(!sp.buf)return false;var px=Math.floor(x/sp.scale),py=Math.floor(y/sp.scale);if(px<0||py<0||px>=W||py>=H)return false;if(sp.flipX)px=W-1-px;if(sp.flipY)py=H-1-py;return !!sp.buf[py*W+px];}
  window.Pixel={Sprite:Sprite,pointer:PTR,opaqueAt:opaqueAt,PAL:PAL};
})();
