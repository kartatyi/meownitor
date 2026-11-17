// Turns a recording into one of the widget's sounds: cut to the sound itself, mono, 22 kHz, brought
// to the loudness all the widget's sounds have, saved as 16-bit WAV — which every webview plays.
//
//     node docs/sound.mjs <recording: mp3, ogg, wav…> <name> [most seconds, 2 by default]
//
// writes app/src/sounds/<name>.wav. Then add the name to FILES in app/src/sounds.js, its words to
// i18n.js (`snd.<name>`) and where it came from to app/src/sounds/README.md.
// Chrome (or Edge) does the decoding, headless — the same browser docs/shoot.py draws with.
import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const [input, name, most = '2'] = process.argv.slice(2);
if (!input || !/^[a-z0-9-]+$/.test(name || '')) {
  console.error('usage: node docs/sound.mjs <recording> <name: a-z, 0-9, -> [most seconds]');
  process.exit(2);
}
const BROWSER = [
  'C:/Program Files/Google/Chrome/Application/chrome.exe',
  'C:/Program Files (x86)/Google/Chrome/Application/chrome.exe',
  'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/usr/bin/google-chrome',
].find(existsSync);
if (!BROWSER) {
  console.error('No Chrome or Edge found');
  process.exit(1);
}

// Runs in the page: decode, mix down to mono at RATE, cut, level, encode.
const convert = async (url, most) => {
  const RATE = 22050, LOUD = 0.25, WIN = Math.round(RATE * 0.05);
  const bytes = await new Promise((done, fail) => {
    const x = new XMLHttpRequest();
    x.open('GET', url);
    x.responseType = 'arraybuffer';
    x.onload = () => done(x.response);
    x.onerror = () => fail(new Error('cannot read ' + url));
    x.send();
  });
  const source = await new OfflineAudioContext(1, 1, 44100).decodeAudioData(bytes);
  const off = new OfflineAudioContext(1, Math.ceil(source.duration * RATE), RATE);
  const node = off.createBufferSource();
  node.buffer = source;
  node.connect(off.destination);
  node.start();
  const all = (await off.startRendering()).getChannelData(0);
  // The sound itself: from the first to the last sample above 2% of the loudest, a few
  // milliseconds kept before it and a short tail after; never longer than `most` seconds.
  let peak = 0;
  for (const v of all) peak = Math.max(peak, Math.abs(v));
  if (!peak) throw new Error('the recording is silent');
  let a = 0, b = all.length - 1;
  while (Math.abs(all[a]) < peak * 0.02) a++;
  while (Math.abs(all[b]) < peak * 0.02) b--;
  a = Math.max(0, a - Math.round(RATE * 0.005));
  b = Math.min(all.length, b + Math.round(RATE * 0.04), a + Math.round(RATE * most));
  const d = all.slice(a, b);
  // No click at either end.
  const fin = Math.min(d.length, Math.round(RATE * 0.003)), fout = Math.min(d.length, Math.round(RATE * 0.03));
  for (let i = 0; i < fin; i++) d[i] *= i / fin;
  for (let i = 0; i < fout; i++) d[d.length - 1 - i] *= i / fout;
  // As loud as the rest: its loudest 50 ms at LOUD (RMS), unless that would clip.
  let loudest = 0;
  for (let i = 0; i + WIN <= d.length; i += WIN >> 1) {
    let s = 0;
    for (let j = i; j < i + WIN; j++) s += d[j] * d[j];
    loudest = Math.max(loudest, Math.sqrt(s / WIN));
  }
  // A recording with one sharp peak would stay quiet if the peak set the limit: past KNEE the
  // samples are bent, not cut, so the peak gives way and the rest comes up.
  const gain = LOUD / (loudest || 1), KNEE = 0.7, ROOM = 0.27;
  const bend = (v) => (Math.abs(v) <= KNEE ? v : Math.sign(v) * (KNEE + ROOM * Math.tanh((Math.abs(v) - KNEE) / ROOM)));
  const wav = new DataView(new ArrayBuffer(44 + d.length * 2));
  const text = (at, s) => [...s].forEach((c, i) => wav.setUint8(at + i, c.charCodeAt(0)));
  text(0, 'RIFF'); wav.setUint32(4, 36 + d.length * 2, true); text(8, 'WAVEfmt ');
  wav.setUint32(16, 16, true); wav.setUint16(20, 1, true); wav.setUint16(22, 1, true);
  wav.setUint32(24, RATE, true); wav.setUint32(28, RATE * 2, true); wav.setUint16(32, 2, true); wav.setUint16(34, 16, true);
  text(36, 'data'); wav.setUint32(40, d.length * 2, true);
  let bent = 0;
  d.forEach((v, i) => {
    const y = bend(v * gain);
    if (y !== v * gain) bent++;
    wav.setInt16(44 + i * 2, Math.round(y * 32767), true);
  });
  let bin = '';
  new Uint8Array(wav.buffer).forEach((v) => { bin += String.fromCharCode(v); });
  return { wav: btoa(bin), seconds: d.length / RATE, was: source.duration, gain, bent: bent / d.length };
};

const here = dirname(fileURLToPath(import.meta.url));
const out = resolve(here, '..', 'app', 'src', 'sounds', name + '.wav');
const tmp = mkdtempSync(join(tmpdir(), 'sound-'));
writeFileSync(join(tmp, 'page.html'), '<!doctype html><title>sound</title>');
const port = 9300 + Math.floor(Math.random() * 500);
const browser = spawn(BROWSER, ['--headless=new', '--disable-gpu', '--allow-file-access-from-files',
  `--remote-debugging-port=${port}`, `--user-data-dir=${join(tmp, 'profile')}`, pathToFileURL(join(tmp, 'page.html')).href], { stdio: 'ignore' });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
try {
  let page;
  for (let i = 0; i < 40 && !page; i++) {
    await sleep(250);
    try { page = (await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()).find((t) => t.type === 'page'); } catch {}
  }
  if (!page) throw new Error('the browser did not start');
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  const answer = new Promise((r) => { ws.onmessage = (m) => { const d = JSON.parse(m.data); if (d.id === 1) r(d.result); }; });
  ws.send(JSON.stringify({ id: 1, method: 'Runtime.evaluate', params: {
    expression: `(${convert})(${JSON.stringify(pathToFileURL(resolve(input)).href)}, ${+most})`, awaitPromise: true, returnByValue: true } }));
  const r = await answer;
  ws.close();
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description || r.exceptionDetails.text);
  const v = r.result.value;
  mkdirSync(dirname(out), { recursive: true });
  const bytes = Buffer.from(v.wav, 'base64');
  writeFileSync(out, bytes);
  console.log(`${name}.wav: ${v.seconds.toFixed(2)} s of ${v.was.toFixed(2)} s, ${Math.round(bytes.length / 1024)} KB, gain ${v.gain.toFixed(2)}, ${(v.bent * 100).toFixed(1)}% of it bent at the peaks`);
} catch (e) {
  console.error(String(e.message || e));
  process.exitCode = 1;
} finally {
  browser.kill();
}
