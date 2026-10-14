/*
  Meownitor round kit — what every question page does (round-kit.css has the look and the
  markup): the Dark/Light switch, picking variants (one per question, several in .q[data-multi]),
  drawings fitted to their cards ([data-w]), numbered markers and frames drawn on the element they
  point at ([data-mark], [data-frame]) or at a screenshot's pixels ([data-px]), a visual opened large,
  and Send — POST /answer to the widget, which hands the answer to the waiting session.

  Load it in <head>, without defer: the theme is set before the first paint, the rest waits until
  the page is parsed (its own scripts have run by then).

  The kit's own words follow the widget's language (it sets window.__ROUND_LANG), or the page's
  <html lang> when the page is opened outside the widget. The answer it sends is always worded the
  same, in English: it is read by Claude, not by the user.
*/
(function () {
  'use strict';
  var doc = document, root = doc.documentElement, KEY = 'round-kit.theme';
  try { var saved = localStorage.getItem(KEY); if (saved === 'dark' || saved === 'light') root.setAttribute('data-theme', saved); } catch (e) { }
  if (!root.getAttribute('data-theme')) root.setAttribute('data-theme', 'dark');

  var VIS = '.vb, .shot, .fig';
  var CIRCLED = '①②③④⑤⑥⑦⑧⑨⑩⑪⑫⑬⑭⑮⑯⑰⑱⑲⑳';
  var ZOOM = '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M9.5 2.5h4v4M13.5 2.5 9.2 6.8M6.5 13.5h-4v-4M2.5 13.5l4.3-4.3"/></svg>';
  var qs = [], msg = null, sendBtn = null, lb = null, frame = 0;

  var WORDS = {
    en: {
      now: 'current', rec: 'recommended', pick: 'Pick {v}', picked: 'Picked', answer: 'Answer',
      comment: 'A comment or your own option (optional) — a click on a number in a picture adds a reference',
      zoom: 'Open larger', prev: 'Previous (←)', next: 'Next (→)', close: 'Close (Esc)',
      others: '← → other options · ', esc: 'Esc to close', theme: 'Theme', send: 'Send',
      progress: 'Answered: {done} of {all}', partial: ' — you can send part of it',
      ready: 'Ready to send.', choose: 'Pick an option and/or write a comment.',
      none: 'Pick at least one option or write a comment.',
      copied: 'Not opened from the widget — the answer is copied, paste it into the chat.',
      nowhere: 'Not opened from the widget — nowhere to send it.',
      sending: 'Sending…', sent: 'Sent — Claude already sees the answer.', failed: 'Could not send — is the widget closed?'
    },
    uk: {
      now: 'як зараз', rec: 'раджу', pick: 'Обрати {v}', picked: 'Обрано', answer: 'Відповідь',
      comment: 'Коментар або свій варіант (необов’язково) — клац по номеру на картинці додасть посилання',
      zoom: 'Відкрити більшим', prev: 'Попередній (←)', next: 'Наступний (→)', close: 'Закрити (Esc)',
      others: '← → інші варіанти · ', esc: 'Esc закрити', theme: 'Тема', send: 'Надіслати',
      progress: 'Відповіді: {done} з {all}', partial: ' — можна надіслати й частину',
      ready: 'Можна надсилати.', choose: 'Обери варіант і/або напиши коментар.',
      none: 'Обери хоча б один варіант або напиши коментар.',
      copied: 'Сторінка відкрита не з віджета — відповідь скопійовано, встав у чат.',
      nowhere: 'Сторінка відкрита не з віджета — надіслати нікуди.',
      sending: 'Надсилаю…', sent: 'Надіслано — Claude уже бачить відповідь.', failed: 'Не вдалося надіслати — віджет закритий?'
    }
  };
  var LANG = window.__ROUND_LANG || ((root.getAttribute('lang') || '').slice(0, 2) === 'uk' ? 'uk' : 'en');
  function word(k, a) {
    var s = (WORDS[LANG] || WORDS.en)[k];
    if (s == null) s = WORDS.en[k];
    return a ? s.replace(/\{(\w+)\}/g, function (m, n) { return a[n] != null ? a[n] : m; }) : s;
  }

  function $(s, el) { return (el || doc).querySelector(s); }
  function $$(s, el) { return [].slice.call((el || doc).querySelectorAll(s)); }
  function make(tag, cls, text) { var e = doc.createElement(tag); if (cls) e.className = cls; if (text != null) e.textContent = text; return e; }
  function clamp(v, a, b) { return Math.max(a, Math.min(b, v)); }
  // the visual an annotation belongs to: the nearest one round it (a visual's own mark goes to the one outside)
  function hostOf(x) { var p = x.matches(VIS) ? x.parentElement : x; return p ? p.closest(VIS) : null; }
  function outer(x) { var o = x, p; while (o.parentElement && (p = o.parentElement.closest(VIS))) o = p; return o; }
  function variants(q) { return $$('.vars > .var', q).filter(function (v) { return !v.parentElement.closest(VIS); }); }
  function comment(q) { return $(':scope > .qc > textarea', q); }
  // the page's own visuals, not the copy in an open lightbox
  function visuals() { return $$(VIS).filter(function (h) { return !h.closest('.lb'); }); }

  if (doc.readyState === 'loading') doc.addEventListener('DOMContentLoaded', init); else init();

  function init() {
    qs = $$('.q');
    qs.forEach(prepQuestion);
    visuals().forEach(prepVisual);
    buildFoot();
    layout();
    progress();
    window.addEventListener('resize', schedule);
    window.addEventListener('load', layout);
    if (doc.fonts && doc.fonts.ready) doc.fonts.ready.then(schedule);
    $$('img').forEach(function (i) { if (!i.complete) i.addEventListener('load', schedule); });
    doc.addEventListener('keydown', onKey);
    doc.addEventListener('click', onMarker);
  }

  // ---- questions and their variants
  function prepQuestion(q, i) {
    if (!q.hasAttribute('data-q')) q.setAttribute('data-q', String(i + 1));
    var id = q.getAttribute('data-q'), multi = q.hasAttribute('data-multi'), h2 = $(':scope > h2', q);
    q._title = h2 ? h2.textContent.trim() : '';
    if (h2 && !$(':scope > .n', h2)) h2.insertBefore(make('span', 'n', id), h2.firstChild);
    var vars = variants(q);
    vars.forEach(function (v, j) {
      if (!v.hasAttribute('data-v')) v.setAttribute('data-v', 'ABCDEFGHIJKL'.charAt(j));
      var letter = v.getAttribute('data-v'), h3 = $(':scope > h3', v);
      if (!h3) { h3 = make('h3'); v.insertBefore(h3, v.firstChild); }
      v._name = h3.textContent.trim();
      h3.insertBefore(make('span', 'lt', letter), h3.firstChild);
      if (v.hasAttribute('data-now')) h3.appendChild(make('span', 'pill now', v.getAttribute('data-now') || word('now')));
      if (v.hasAttribute('data-rec')) h3.appendChild(make('span', 'pill rec', v.getAttribute('data-rec') || word('rec')));
      if (!$(VIS, v)) v.classList.add('text');
      var pk = make('div', 'pk'), b = make('button');
      b.type = 'button'; pk.appendChild(b); v.appendChild(pk);
      v.tabIndex = 0;
      v.setAttribute('role', multi ? 'checkbox' : 'radio');
      setPicked(v, false);
      v.addEventListener('click', function (e) {
        if (e.target.closest('a, input, textarea, select, .mkn, .zb')) return;
        // a click on the drawing opens it large; on the rest of the card it picks the variant
        var vis = e.target.closest(VIS);
        if (vis && v.contains(vis)) { var top = outer(vis); if (!top.hasAttribute('data-live')) openLb(top); return; }
        toggle(q, v);
      });
      v.addEventListener('keydown', function (e) {
        if (e.target === v && (e.key === ' ' || e.key === 'Enter') && !e.ctrlKey) { e.preventDefault(); toggle(q, v); }
      });
    });
    var box = $('.vars', q);
    if (box && vars.length && vars.every(function (v) { return v.classList.contains('text'); })) box.classList.add('text');
    var ta = $(':scope > textarea', q);
    if (ta || !comment(q)) {
      var qc = make('div', 'qc');
      if (!ta) {
        ta = make('textarea');
        ta.placeholder = vars.length ? word('comment') : word('answer');
      }
      q.appendChild(qc); qc.appendChild(ta);
    }
    ta = comment(q);
    ta.rows = 1;
    ta.addEventListener('input', function () { grow(ta); progress(); });
    grow(ta);
  }

  function toggle(q, v) {
    var on = !v.classList.contains('picked');
    if (!q.hasAttribute('data-multi')) variants(q).forEach(function (x) { if (x !== v) setPicked(x, false); });
    setPicked(v, on);
    progress();
    if (lb) lbHead();
  }

  function setPicked(v, on) {
    v.classList.toggle('picked', on);
    v.setAttribute('aria-checked', on ? 'true' : 'false');
    var b = $(':scope > .pk > button', v);
    if (b) b.textContent = on ? word('picked') : word('pick', { v: v.getAttribute('data-v') });
  }

  function grow(ta) {
    ta.style.height = 'auto';
    ta.style.height = (ta.scrollHeight + ta.offsetHeight - ta.clientHeight) + 'px';
  }

  // ---- visuals: the drawn-at-width stage, the zoom button
  function prepVisual(h) {
    var w = +h.getAttribute('data-w');
    if (w && !$(':scope > .zs', h)) {
      var zs = make('div', 'zs'), zi = make('div', 'zi');
      [].slice.call(h.childNodes).forEach(function (n) {
        if (!(n.nodeType === 1 && n.matches('.mkn, .frame'))) zi.appendChild(n);
      });
      zi.style.width = w + 'px';
      zs.appendChild(zi);
      h.insertBefore(zs, h.firstChild);
    }
    if (outer(h) !== h) return;
    var b = make('button', 'zb');
    b.type = 'button'; b.title = word('zoom'); b.innerHTML = ZOOM;
    b.addEventListener('click', function (e) { e.stopPropagation(); openLb(h); });
    h.appendChild(b);
    if (!h.closest('.var')) h.addEventListener('click', function (e) {
      if (h.hasAttribute('data-live') || e.target.closest('a, input, textarea, select, button, .mkn')) return;
      openLb(h);
    });
  }

  function schedule() { if (!frame) frame = requestAnimationFrame(function () { frame = 0; layout(); }); }

  function layout() {
    $$('.vars').forEach(columns);
    visuals().forEach(fit);
    visuals().forEach(function (h) { fromPixels(h); place(h); });
    if (lb) lbShow();
  }

  // three across, two when there are two or four, fewer when the window is narrow
  function columns(box) {
    if (box.classList.contains('text')) return;
    var n = $$(':scope > .var:not(.wide)', box).length || 1;
    var want = +box.getAttribute('data-cols') || (n === 4 ? 2 : Math.min(n, 3));
    var room = Math.max(1, Math.floor((box.clientWidth + 16) / (300 + 16)));
    box.style.gridTemplateColumns = 'repeat(' + Math.max(1, Math.min(want, room)) + ', minmax(0, 1fr))';
  }

  // [data-w]: the drawing is laid out that wide and scaled down to the room it has — never up
  function fit(h) {
    var zs = $(':scope > .zs', h);
    if (!zs) return;
    var zi = zs.firstChild, w = +h.getAttribute('data-w'), cw = zs.clientWidth;
    if (!cw || !w) return;
    var s = Math.min(1, cw / w);
    zi.style.transform = 'scale(' + s + ')';
    zi.style.marginLeft = s < 1 ? '0' : Math.floor((cw - w) / 2) + 'px';
    zs.style.height = Math.ceil(zi.offsetHeight * s) + 'px';
    h._s = s;
  }

  // .mkn / .frame with data-px="x y [w h]" in the screenshot's own pixels → percent of the picture
  function fromPixels(h) {
    var marks = $$(':scope > [data-px]', h);
    if (!marks.length) return;
    var img = $$('img', h).filter(function (i) { return i.naturalWidth; })[0];
    if (!img) return;
    marks.forEach(function (a) {
      var v = a.getAttribute('data-px').trim().split(/[\s,]+/).map(Number);
      a.style.left = v[0] / img.naturalWidth * 100 + '%';
      a.style.top = v[1] / img.naturalHeight * 100 + '%';
      if (v.length > 3) { a.style.width = v[2] / img.naturalWidth * 100 + '%'; a.style.height = v[3] / img.naturalHeight * 100 + '%'; }
    });
  }

  // [data-frame] and [data-mark] on elements of a drawing: the frame round the element, the number
  // at its corner (data-at: tl tr bl br t b l r c), on a layer of the visual that is never scaled
  function place(h) {
    var al = $(':scope > .al', h), hr = h.getBoundingClientRect();
    var targets = $$('[data-mark], [data-frame]', h).filter(function (t) { return hostOf(t) === h; });
    if (al) al.textContent = '';
    if (targets.length && hr.width) {
      if (!al) { al = make('div', 'al'); h.appendChild(al); }
      var k = hr.width / h.offsetWidth || 1, W = h.clientWidth, H = h.clientHeight;
      var pad = function (t) { return t.hasAttribute('data-pad') ? +t.getAttribute('data-pad') : 4; };
      var box = function (t, p) {
        var r = t.getBoundingClientRect();
        if (!r.width && !r.height) return null;
        return { x: (r.left - hr.left) / k - h.clientLeft - p, y: (r.top - hr.top) / k - h.clientTop - p, w: r.width / k + 2 * p, h: r.height / k + 2 * p };
      };
      targets.forEach(function (t) {
        if (!t.hasAttribute('data-frame')) return;
        var b = box(t, pad(t));
        if (!b) return;
        var f = make('i', 'frame'), label = t.getAttribute('data-frame');
        f.style.cssText = 'left:' + b.x + 'px;top:' + b.y + 'px;width:' + b.w + 'px;height:' + b.h + 'px';
        if (label) f.setAttribute('data-label', label);
        if (t.hasAttribute('data-dim')) f.setAttribute('data-dim', '');
        if (t.hasAttribute('data-dash')) f.classList.add('dash');
        al.appendChild(f);
      });
      targets.forEach(function (t) {
        if (!t.hasAttribute('data-mark')) return;
        var b = box(t, t.hasAttribute('data-frame') ? pad(t) : 0);
        if (!b) return;
        var at = t.getAttribute('data-at') || 'tl', o = 3, out = 15;
        var x = /l$/.test(at) ? b.x - (at === 'l' ? out : o) : /r$/.test(at) ? b.x + b.w + (at === 'r' ? out : o) : b.x + b.w / 2;
        var y = at.charAt(0) === 't' ? b.y - (at === 't' ? out : o) : at.charAt(0) === 'b' ? b.y + b.h + (at === 'b' ? out : o) : b.y + b.h / 2;
        var m = make('i', 'mkn', t.getAttribute('data-mark'));
        m.style.left = clamp(x, 13, W - 13) + 'px';
        m.style.top = clamp(y, 13, H - 13) + 'px';
        al.appendChild(m);
      });
    }
    // a frame's tag goes under it when there is no room above
    $$(':scope > .frame[data-label], :scope > .al > .frame[data-label]', h).forEach(function (f) {
      f.classList.toggle('below', !f.classList.contains('inside') && f.offsetTop < 26);
    });
  }

  // a click on a number in a picture puts «A②» into that question's comment
  function onMarker(e) {
    var m = e.target.closest('.mkn');
    if (!m || !m.parentElement || !m.parentElement.matches('.al, ' + VIS)) return;
    var src = lb && lb.el.contains(m) ? lb.list[lb.i] : m, q = src.closest('.q');
    if (!q) return;
    var v = src.closest('.var'), n = m.textContent.trim(), ta = comment(q);
    var num = /^\d+$/.test(n) && +n >= 1 && +n <= 20 ? CIRCLED.charAt(+n - 1) : '#' + n;
    if (lb) closeLb();
    ta.value = ta.value.replace(/\s+$/, '');
    ta.value += (ta.value ? '\n' : '') + (v ? v.getAttribute('data-v') : '') + num + ' ';
    grow(ta);
    progress();
    ta.focus();
    ta.setSelectionRange(ta.value.length, ta.value.length);
  }

  // ---- a visual opened large; ‹ › walk the question's variants
  function openLb(h) {
    var q = h.closest('.q'), list = [h];
    if (q && h.closest('.var')) {
      list = variants(q).map(function (v) { var x = $(VIS, v); return x && outer(x); }).filter(Boolean);
      if (list.indexOf(h) < 0) list = [h];
    }
    if (!lb) {
      var el = make('div', 'lb');
      el.setAttribute('role', 'dialog');
      el.innerHTML = '<div class="lb-bar"><div class="t"></div><span class="hint"></span>' +
        '<button type="button" class="lb-prev" title="' + word('prev') + '">‹</button><button type="button" class="lb-next" title="' + word('next') + '">›</button>' +
        '<button type="button" class="lb-pick"></button><button type="button" class="lb-x" title="' + word('close') + '">✕</button></div><div class="lb-body"></div>';
      doc.body.appendChild(el);
      lb = { el: el };
      $('.lb-x', el).addEventListener('click', closeLb);
      $('.lb-prev', el).addEventListener('click', function () { step(-1); });
      $('.lb-next', el).addEventListener('click', function () { step(1); });
      $('.lb-pick', el).addEventListener('click', function () { var v = lb.list[lb.i].closest('.var'); if (v) toggle(v.closest('.q'), v); });
      $('.lb-body', el).addEventListener('click', function (e) { if (e.target === e.currentTarget) closeLb(); });
      root.style.overflow = 'hidden';
    }
    lb.list = list;
    lb.i = list.indexOf(h);
    lbShow();
  }

  function step(d) {
    var i = lb.i + d;
    if (i < 0 || i >= lb.list.length) return;
    lb.i = i;
    lbShow();
  }

  function lbHead() {
    var h = lb.list[lb.i], v = h.closest('.var'), t = $('.t', lb.el), pick = $('.lb-pick', lb.el), q = h.closest('.q');
    t.textContent = '';
    if (v) {
      t.appendChild(make('span', 'lt', v.getAttribute('data-v')));
      t.appendChild(doc.createTextNode(v._name || ''));
    } else t.textContent = (q && q._title) || doc.title;
    t.classList.toggle('picked', !!v && v.classList.contains('picked'));
    pick.style.display = v ? '' : 'none';
    if (v) {
      var on = v.classList.contains('picked');
      pick.textContent = on ? word('picked') : word('pick', { v: v.getAttribute('data-v') });
      pick.classList.toggle('on', on);
    }
    var many = lb.list.length > 1;
    $('.lb-prev', lb.el).style.display = $('.lb-next', lb.el).style.display = many ? '' : 'none';
    $('.lb-prev', lb.el).disabled = lb.i === 0;
    $('.lb-next', lb.el).disabled = lb.i === lb.list.length - 1;
    $('.hint', lb.el).textContent = (many ? word('others') : '') + word('esc');
  }

  // the clone is laid out exactly as on the page and scaled as a whole; its annotations are drawn
  // again at that scale and kept at their own size (--ik)
  function lbShow() {
    lbHead();
    var h = lb.list[lb.i], body = $('.lb-body', lb.el);
    body.textContent = '';
    var W = h.offsetWidth, H = h.offsetHeight;
    if (!W || !H) return;
    var c = h.cloneNode(true);
    $$('.zb, .al', c).forEach(function (x) { x.remove(); });
    var from = $$('canvas', h);
    $$('canvas', c).forEach(function (x, j) { try { x.getContext('2d').drawImage(from[j], 0, 0); } catch (e) { } });
    // as large as the window allows, but a drawing no larger than it was drawn and a screenshot no
    // larger than its own pixels
    var max = 2, img = $$('img', h).filter(function (i) { return i.offsetWidth && i.naturalWidth; })[0];
    if (h._s) max = 1 / h._s;
    else if (img) max = img.naturalWidth / img.offsetWidth;
    var k = Math.min((body.clientWidth - 48) / W, (body.clientHeight - 48) / H, Math.max(1, max));
    var box = make('div', 'lb-fit');
    box.style.width = W * k + 'px';
    box.style.height = H * k + 'px';
    c.style.width = W + 'px';
    c.style.height = H + 'px';
    c.style.transform = 'scale(' + k + ')';
    c.style.setProperty('--ik', String(1 / k));
    box.appendChild(c);
    body.appendChild(box);
    var again = function () { [c].concat($$(VIS, c)).forEach(place); };
    again();
    // the copy's pictures lay out once they are decoded: draw its annotations again then
    $$('img', c).forEach(function (i) { if (!i.complete) i.addEventListener('load', again); });
  }

  function closeLb() {
    if (!lb) return;
    lb.el.remove();
    lb = null;
    root.style.overflow = '';
  }

  function onKey(e) {
    if (lb) {
      if (e.key === 'Escape') { e.preventDefault(); closeLb(); }
      else if (e.key === 'ArrowLeft') { e.preventDefault(); step(-1); }
      else if (e.key === 'ArrowRight') { e.preventDefault(); step(1); }
      return;
    }
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) { e.preventDefault(); send(); }
  }

  // ---- the bar at the foot: the theme, what is answered, Send
  function buildFoot() {
    var f = make('div', 'foot');
    f.innerHTML = '<div class="seg" role="group" aria-label="' + word('theme') + '"><button type="button" data-th="dark">Dark</button><button type="button" data-th="light">Light</button></div>' +
      '<span class="msg"></span><span class="kbd">Ctrl+Enter</span><button type="button" class="send">' + word('send') + '</button>';
    doc.body.appendChild(f);
    msg = $('.msg', f);
    sendBtn = $('.send', f);
    sendBtn.addEventListener('click', send);
    $$('[data-th]', f).forEach(function (b) {
      b.classList.toggle('on', b.getAttribute('data-th') === root.getAttribute('data-theme'));
      b.addEventListener('click', function () {
        var t = b.getAttribute('data-th');
        root.setAttribute('data-theme', t);
        try { localStorage.setItem(KEY, t); } catch (e) { }
        $$('[data-th]', f).forEach(function (x) { x.classList.toggle('on', x === b); });
        schedule();
      });
    });
  }

  function answered(q) {
    return variants(q).some(function (v) { return v.classList.contains('picked'); }) || !!comment(q).value.trim();
  }

  function say(kind, text) { msg.className = 'msg' + (kind ? ' ' + kind : ''); msg.textContent = text; }

  function progress() {
    var done = qs.filter(function (q) { var a = answered(q); q.classList.toggle('answered', a); return a; }).length;
    if (!msg || (sendBtn && sendBtn.disabled)) return;
    if (qs.length > 1) say('', word('progress', { done: done, all: qs.length }) + (done && done < qs.length ? word('partial') : ''));
    else say('', done ? word('ready') : word('choose'));
  }

  // «1 — A» per question (several picks «2 — A, C», no pick but a comment «3 — own option»), the
  // comment indented under it — in English whatever the page's language, so a session always reads
  // the same shape
  function send() {
    if (!sendBtn || sendBtn.disabled) return;
    var lines = [doc.title.trim()], any = false;
    qs.forEach(function (q) {
      var vars = variants(q), id = q.getAttribute('data-q'), t = comment(q).value.trim();
      var picked = vars.filter(function (v) { return v.classList.contains('picked'); }).map(function (v) { return v.getAttribute('data-v'); });
      if (vars.length) lines.push(id + ' — ' + (picked.length ? picked.join(', ') : t ? 'own option' : 'no pick'));
      else lines.push(id + ':' + (t ? '' : ' no answer'));
      if (picked.length || t) any = true;
      if (t) lines.push('  comment: ' + t.replace(/\r?\n/g, '\n  '));
    });
    if (!any) { say('err', word('none')); return; }
    var text = lines.join('\n');
    if (location.protocol === 'file:') {
      var copied = function () { say('err', word('copied')); };
      try { navigator.clipboard.writeText(text).then(copied, function () { say('err', word('nowhere')); }); } catch (e) { say('err', word('nowhere')); }
      return;
    }
    sendBtn.disabled = true;
    say('', word('sending'));
    fetch('/answer', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ page: location.pathname, text: text }) })
      .then(function (r) { if (!r.ok) throw 0; say('ok', word('sent')); })
      .catch(function () { sendBtn.disabled = false; say('err', word('failed')); });
  }
})();
