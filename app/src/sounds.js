// The widget's sounds. Some are recordings (sounds/*.wav next to this file — where each came from
// is in sounds/README.md), fetched and decoded once and then played from memory. The rest are made
// on the spot with Web Audio, a few notes each. Which sound goes with what is the card's business
// (widget.js); here is only what there is to play and how.
(function(){
  'use strict';
  var HERE=((document.currentScript&&document.currentScript.src)||'').replace(/[^\/]*$/,'');
  var ctx=null,limiter=null,heard={};
  function context(){
    ctx=ctx||new (window.AudioContext||window.webkitAudioContext)();
    if(ctx.state==='suspended')ctx.resume();
    return ctx;
  }
  // Everything goes out through a limiter, so a sound turned all the way up is squeezed, not clipped.
  function master(c){
    if(!limiter){
      limiter=c.createDynamicsCompressor();
      limiter.threshold.value=-9;limiter.knee.value=6;limiter.ratio.value=12;limiter.attack.value=.003;limiter.release.value=.12;
      limiter.connect(c.destination);
    }
    return limiter;
  }

  // The recordings, by name: all cut to the sound itself and brought to one loudness when they
  // were added (docs/sound.mjs does that), so they play as they are.
  var FILES={meow:'sounds/meow.wav',meow2:'sounds/meow2.wav',meow3:'sounds/meow3.wav',quack:'sounds/quack.wav',quack2:'sounds/quack2.wav',frog:'sounds/frog.wav',cluck:'sounds/cluck.wav',plop:'sounds/plop.wav'};
  function recording(c,name){
    if(!heard[name])heard[name]=fetch(HERE+FILES[name]).then(function(r){if(!r.ok)throw 0;return r.arrayBuffer();}).then(function(b){return c.decodeAudioData(b);});
    return heard[name];
  }

  // The made ones. A value over time is [[seconds from t, value], …]: pitches glide, loudness ramps.
  function glide(param,t,pts){param.setValueAtTime(pts[0][1],t+pts[0][0]);for(var i=1;i<pts.length;i++)param.exponentialRampToValueAtTime(pts[i][1],t+pts[i][0]);}
  function shape(c,t,pts){var g=c.createGain();g.gain.setValueAtTime(0,t);pts.forEach(function(p){g.gain.linearRampToValueAtTime(p[1],t+p[0]);});return g;}
  function amp(c,v){var g=c.createGain();g.gain.value=v;return g;}
  function osc(c,type,t,dur){var o=c.createOscillator();o.type=type;o.start(t);o.stop(t+dur+.06);return o;}
  // A plain note: a quick attack and a tail that dies away.
  function note(c,out,t,type,f,dur,vol){
    var o=osc(c,type,t,dur),g=c.createGain();
    o.frequency.setValueAtTime(f,t);
    g.gain.setValueAtTime(0,t);g.gain.linearRampToValueAtTime(vol,t+.012);g.gain.exponentialRampToValueAtTime(.001,t+dur);
    o.connect(g);g.connect(out);
  }
  var PATCH={
    // «cu-ckoo»: two flute notes, the second a third lower.
    cuckoo:function(c,out,t){
      [[0,659,.24],[.3,523,.34]].forEach(function(n){
        var o=osc(c,'sine',t+n[0],n[2]),h=osc(c,'triangle',t+n[0],n[2]),e=shape(c,t+n[0],[[.03,1],[n[2]-.08,.8],[n[2],0]]),m=amp(c,.25);
        o.frequency.value=n[1];h.frequency.value=n[1]*2;o.connect(e);h.connect(m);m.connect(e);e.connect(out);
      });
    },
    // «pew»: a toy laser, sliding all the way down.
    pew:function(c,out,t){
      var d=.26,o=osc(c,'square',t,d),e=shape(c,t,[[.01,.5],[.2,.35],[d,0]]);
      glide(o.frequency,t,[[0,1700],[d,170]]);o.connect(e);e.connect(out);
    },
    // A coin from an old game: two square notes, the second ringing.
    coin:function(c,out,t){note(c,out,t,'square',988,.09,.5);note(c,out,t+.085,'square',1319,.42,.5);},
    // The chirp the widget has always had: two rising notes.
    chirp:function(c,out,t){note(c,out,t,'sine',660,.16,1);note(c,out,t+.14,'sine',880,.24,1);}
  };
  // What each made sound is multiplied by to come out as loud as the recordings do (measured: the
  // loudest 50 ms of each, rendered at 1, is about 0.25 — a square wave carries far more than a sine).
  var EVEN={cuckoo:.36,pew:.75,coin:.72,chirp:.6};

  // Plays `name` at `vol` (0…1); one the widget doesn't have plays as the chirp. `to` — an audio
  // context and a node of it — is for a page that renders the made sounds to measure them.
  function play(name,vol,to){
    try{
      var c=to?to.ctx:context(),out=c.createGain();
      vol=Math.max(0,Math.min(1,vol));
      out.connect(to?to.node:master(c));
      if(FILES[name]&&!to){
        out.gain.value=vol;
        recording(c,name).then(function(buf){var s=c.createBufferSource();s.buffer=buf;s.connect(out);s.start();}).catch(function(){delete heard[name];});
        return;
      }
      if(!PATCH[name])name='chirp';
      out.gain.value=vol*EVEN[name];
      PATCH[name](c,out,c.currentTime+.02);
    }catch(e){}
  }
  // The recordings first: they are what one comes for.
  window.Sounds={NAMES:Object.keys(FILES).concat(Object.keys(PATCH)),play:play};
})();
