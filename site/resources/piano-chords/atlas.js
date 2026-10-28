// BEGIN PURE
const LETTERS = ['C','D','E','F','G','A','B'];
const NAT_PC = [0,2,4,5,7,9,11];
const ACC = { '-2':'\u{1D12B}', '-1':'♭', '0':'', '1':'♯', '2':'\u{1D12A}' };
const ROOTS = [
  {name:'C', letter:0, pc:0}, {name:'D♭', letter:1, pc:1}, {name:'D', letter:1, pc:2},
  {name:'E♭', letter:2, pc:3}, {name:'E', letter:2, pc:4}, {name:'F', letter:3, pc:5},
  {name:'F♯', letter:3, pc:6}, {name:'G', letter:4, pc:7}, {name:'A♭', letter:5, pc:8},
  {name:'A', letter:5, pc:9}, {name:'B♭', letter:6, pc:10}, {name:'B', letter:6, pc:11}
];
const FAMILIES = [
  {id:'triads', label:'Triads'}, {id:'sixths', label:'Sixths & adds'}, {id:'sevenths', label:'Sevenths'},
  {id:'extended', label:'Extended'}, {id:'altered', label:'Altered'}
];
const TWO_HAND_FAMS = ['sevenths','extended','altered'];
// f = semitones, d = scale degree of each tone (drives letter spelling), x = degrees shown as alterations
const TYPES = [
  {id:'maj', sym:'', name:'Major', fam:'triads', f:[0,4,7], d:[1,3,5], use:'Bright, stable, the home sound'},
  {id:'min', sym:'m', name:'Minor', fam:'triads', f:[0,3,7], d:[1,3,5], use:'Darker, sad or serious'},
  {id:'5', sym:'5', name:'Power chord', fam:'triads', f:[0,7], d:[1,5], use:'Neutral and punchy; rock staple'},
  {id:'sus2', sym:'sus2', name:'Suspended 2nd', fam:'triads', f:[0,2,7], d:[1,2,5], use:'Open and airy, no third'},
  {id:'sus4', sym:'sus4', name:'Suspended 4th', fam:'triads', f:[0,5,7], d:[1,4,5], use:'Tension that wants to fall to major'},
  {id:'aug', sym:'aug', name:'Augmented', fam:'triads', f:[0,4,8], d:[1,3,5], use:'Uneasy and rising; a passing chord'},
  {id:'dim', sym:'dim', name:'Diminished', fam:'triads', f:[0,3,6], d:[1,3,5], use:'Tense and unstable; leads by half step'},

  {id:'6', sym:'6', name:'Major 6th', fam:'sixths', f:[0,4,7,9], d:[1,3,5,6], use:'Sweet vintage major; swing endings'},
  {id:'m6', sym:'m6', name:'Minor 6th', fam:'sixths', f:[0,3,7,9], d:[1,3,5,6], use:'Moody minor with a noir edge'},
  {id:'69', sym:'6/9', name:'Six-nine', fam:'sixths', f:[0,4,7,9,14], d:[1,3,5,6,9], use:'Lush open major; jazz and bossa'},
  {id:'add9', sym:'add9', name:'Add nine', fam:'sixths', f:[0,4,7,14], d:[1,3,5,9], use:'Major with sparkle; common in pop'},
  {id:'madd9', sym:'m(add9)', name:'Minor add nine', fam:'sixths', f:[0,3,7,14], d:[1,3,5,9], use:'Wistful minor with extra shimmer'},

  {id:'7', sym:'7', name:'Dominant 7th', fam:'sevenths', f:[0,4,7,10], d:[1,3,5,7], use:'Bluesy dominant; wants to resolve'},
  {id:'maj7', sym:'maj7', name:'Major 7th', fam:'sevenths', f:[0,4,7,11], d:[1,3,5,7], use:'Soft, dreamy, settled major'},
  {id:'m7', sym:'m7', name:'Minor 7th', fam:'sevenths', f:[0,3,7,10], d:[1,3,5,7], use:'Mellow minor; soul and jazz workhorse'},
  {id:'mmaj7', sym:'m(maj7)', name:'Minor major 7th', fam:'sevenths', f:[0,3,7,11], d:[1,3,5,7], use:'Dark and suspenseful; spy film sound'},
  {id:'m7b5', sym:'m7♭5', name:'Half-diminished', fam:'sevenths', f:[0,3,6,10], d:[1,3,5,7], use:'The ii chord in minor keys'},
  {id:'dim7', sym:'dim7', name:'Diminished 7th', fam:'sevenths', f:[0,3,6,9], d:[1,3,5,7], use:'Symmetrical tension; slides anywhere'},
  {id:'7sus4', sym:'7sus4', name:'Dominant 7th sus4', fam:'sevenths', f:[0,5,7,10], d:[1,4,5,7], use:'Dominant pull without the third'},
  {id:'7sharp5', sym:'7♯5', name:'Augmented 7th', fam:'sevenths', f:[0,4,8,10], d:[1,3,5,7], x:[5], use:'Dominant with a restless rising top'},
  {id:'7b5', sym:'7♭5', name:'Dominant 7th flat 5', fam:'sevenths', f:[0,4,6,10], d:[1,3,5,7], x:[5], use:'Dominant with a whole-tone tang'},

  {id:'9', sym:'9', name:'Dominant 9th', fam:'extended', f:[0,4,7,10,14], d:[1,3,5,7,9], use:'Rich dominant; funk and blues'},
  {id:'maj9', sym:'maj9', name:'Major 9th', fam:'extended', f:[0,4,7,11,14], d:[1,3,5,7,9], use:'Smooth glossy major; neo-soul color'},
  {id:'m9', sym:'m9', name:'Minor 9th', fam:'extended', f:[0,3,7,10,14], d:[1,3,5,7,9], use:'Warm, deep minor; R&B and jazz'},
  {id:'11', sym:'11', name:'Dominant 11th', fam:'extended', f:[0,7,10,14,17], d:[1,5,7,9,11], use:'Wide open dominant; gospel and fusion'},
  {id:'m11', sym:'m11', name:'Minor 11th', fam:'extended', f:[0,3,7,10,14,17], d:[1,3,5,7,9,11], use:'Spacious minor; modal jazz feel'},
  {id:'13', sym:'13', name:'Dominant 13th', fam:'extended', f:[0,4,10,14,21], d:[1,3,7,9,13], use:'Full swinging dominant; big-band sound'},
  {id:'m13', sym:'m13', name:'Minor 13th', fam:'extended', f:[0,3,10,14,21], d:[1,3,7,9,13], use:'Dorian minor with a bright top'},
  {id:'maj13', sym:'maj13', name:'Major 13th', fam:'extended', f:[0,4,11,14,21], d:[1,3,7,9,13], use:'Lush wide major; ballad endings'},

  {id:'7b9', sym:'7♭9', name:'Dominant 7th flat 9', fam:'altered', f:[0,4,7,10,13], d:[1,3,5,7,9], use:'Dark dominant pulling hard to minor'},
  {id:'7sharp9', sym:'7♯9', name:'Dominant 7th sharp 9', fam:'altered', f:[0,4,7,10,15], d:[1,3,5,7,9], use:'Gritty blues-rock dominant; the Hendrix chord'},
  {id:'maj7sharp11', sym:'maj7♯11', name:'Major 7th sharp 11', fam:'altered', f:[0,4,7,11,18], d:[1,3,5,7,11], use:'Floating Lydian major; modern and bright'},
  {id:'7sharp11', sym:'7♯11', name:'Dominant 7th sharp 11', fam:'altered', f:[0,4,7,10,18], d:[1,3,5,7,11], use:'Lydian dominant; bright open tension'},
  {id:'7alt', sym:'7alt (7♯5♯9)', name:'Altered dominant', fam:'altered', f:[0,4,8,10,15], d:[1,3,5,7,9], x:[5], use:'Maximum tension before resolving home'}
];
const TYPE_BY_ID = Object.fromEntries(TYPES.map(t => [t.id, t]));
const NAT_SEMI = {1:0, 2:2, 3:4, 4:5, 5:7, 6:9, 7:11, 9:14, 11:17, 13:21};
const BLACK_PCS = [1,3,6,8,10];

function mod12(n){ return ((n % 12) + 12) % 12; }
function isBlack(abs){ return BLACK_PCS.includes(mod12(abs)); }
function degreeLabel(semis, deg){ return ACC[String(semis - NAT_SEMI[deg])] + deg; }
function spellNote(rootIdx, semis, deg){
  const r = ROOTS[rootIdx];
  const pc = mod12(r.pc + semis);
  const letter = (r.letter + deg - 1) % 7;
  let acc = mod12(pc - NAT_PC[letter]);
  if (acc > 6) acc -= 12;
  const sym = ACC[String(acc)];
  return { name: LETTERS[letter] + (sym === undefined ? '?' + acc : sym), pc, letter, acc };
}
function chordTones(rootIdx, typeId){
  const t = TYPE_BY_ID[typeId];
  return t.f.map((s, i) => {
    const deg = t.d[i];
    const role = i === 0 ? 'root' : (deg >= 9 || (t.x && t.x.includes(deg))) ? 'ext' : 'tone';
    return Object.assign({ semis:s, deg, label:degreeLabel(s, deg), role }, spellNote(rootIdx, s, deg));
  });
}
// Which chord tone goes in the bass for each inversion
const INV_TARGETS = [[1], [3,2,4], [5], [7,6]];
const INV_NEED = ['root', '3rd', '5th', '7th or 6th'];
function inversionIndex(typeId, inv){
  const t = TYPE_BY_ID[typeId];
  for (const d of INV_TARGETS[inv]) { const i = t.d.indexOf(d); if (i >= 0) return i; }
  return -1;
}
function voiceInversion(rootIdx, typeId, inv){
  const tones = chordTones(rootIdx, typeId);
  let idx = inversionIndex(typeId, inv);
  const applied = idx >= 0;
  if (!applied) idx = 0;
  const sb = tones[idx].semis;
  const rp = ROOTS[rootIdx].pc;
  const keys = tones.map(t => { let p = t.semis; while (p < sb) p += 12; return Object.assign({}, t, { abs: rp + p }); })
    .sort((a, b) => a.abs - b.abs);
  const lo = keys[0].abs, hi = keys[keys.length - 1].abs;
  const start = Math.floor(lo / 12) * 12;
  const octaves = Math.max(2, Math.floor((hi - start) / 12) + 1);
  return { keys, applied, bass: tones[idx], range: { start, end: start + octaves * 12 - 1 } };
}
function twoHandVoicing(rootIdx, typeId){
  const tones = chordTones(rootIdx, typeId);
  const rp = ROOTS[rootIdx].pc;
  const lhRootAbs = rp >= 5 ? rp - 12 : rp;
  const root = tones[0];
  const guide = tones.find(t => t.deg === 7) || tones.find(t => t.deg === 5);
  const lh = [ Object.assign({}, root, { abs: lhRootAbs, hand:'L' }), Object.assign({}, guide, { abs: lhRootAbs + guide.semis % 12, hand:'L' }) ];
  let rest = tones.filter(t => t !== root && t !== guide);
  if (rest.length >= 4) rest = rest.filter(t => !(t.deg === 5 && t.semis === 7));
  const first = rest.find(t => t.deg === 3) || rest.find(t => t.deg === 4) || rest[0];
  const lhTop = lh[1].abs;
  let a = lhTop + 1; while (mod12(a) !== first.pc) a++;
  const others = rest.filter(t => t !== first).sort((x, y) => mod12(x.pc - first.pc) - mod12(y.pc - first.pc));
  const rh = [ Object.assign({}, first, { abs:a, hand:'R' }) ];
  let prev = a;
  for (const t of others) { let b = prev + 1; while (mod12(b) !== t.pc) b++; rh.push(Object.assign({}, t, { abs:b, hand:'R' })); prev = b; }
  const keys = lh.concat(rh).sort((x, y) => x.abs - y.abs);
  const lo = keys[0].abs, hi = keys[keys.length - 1].abs;
  let start = lo; while (![0,5].includes(mod12(start))) start--;
  let end = hi; while (![4,11].includes(mod12(end))) end++;
  while (end - start + 1 < 24) { end++; while (![4,11].includes(mod12(end))) end++; }
  return { keys, lh, rh, omitted: tones.filter(t => !keys.some(k => k.deg === t.deg)), range: { start, end } };
}
function rhFingers(s){
  const n = s.length;
  if (n === 1) return '1';
  if (n === 2) return (s[1] - s[0]) <= 4 ? '1-3' : '1-5';
  if (n === 3) return '1-3-5';
  if (n === 4) return '1-2-3-5';
  return '1-2-3-4-5';
}
function fingering(typeId){
  const s = TYPE_BY_ID[typeId].f.slice().sort((a, b) => a - b);
  if (s[s.length - 1] - s[0] <= 12) return { lh:[], lhF:'', rh:s, rhF:rhFingers(s) };
  let k = 1; while (s[s.length - 1] - s[k] > 12) k++;
  const lh = s.slice(0, k), rh = s.slice(k);
  const lhF = lh.length === 1 ? '5' : lh.length === 2 ? ((lh[1] - lh[0]) <= 4 ? '5-3' : '5-1') : '5-3-1';
  return { lh, lhF, rh, rhF:rhFingers(rh) };
}
// END PURE

(function(){
  const store = {
    get(k){ try { return localStorage.getItem('pca.' + k); } catch(e) { return null; } },
    set(k, v){ try { localStorage.setItem('pca.' + k, String(v)); } catch(e) {} }
  };
  const state = { root:0, fam:'all', inv:0 };
  const r = parseInt(store.get('root'), 10); if (r >= 0 && r < 12) state.root = r;
  const f = store.get('fam'); if (f === 'all' || FAMILIES.some(x => x.id === f)) state.fam = f;
  const v = parseInt(store.get('inv'), 10); if (v >= 0 && v <= 3) state.inv = v;

  const INV_LABELS = ['Root', '1st', '2nd', '3rd'];
  const INV_NAMES = ['Root position', '1st inversion', '2nd inversion', '3rd inversion'];
  const $ = id => document.getElementById(id);

  $('roots').innerHTML = ROOTS.map((x, i) => `<button type="button" data-root="${i}" aria-label="Root ${x.name}">${x.name}</button>`).join('');
  $('fams').innerHTML = [{id:'all', label:'All'}].concat(FAMILIES).map(x => `<button type="button" data-fam="${x.id}">${x.label}</button>`).join('');
  $('invs').innerHTML = INV_LABELS.map((x, i) => `<button type="button" data-inv="${i}" aria-label="${INV_NAMES[i]}">${x}</button>`).join('');

  $('roots').addEventListener('click', e => { const b = e.target.closest('button'); if (!b) return; state.root = +b.dataset.root; store.set('root', state.root); render(); });
  $('fams').addEventListener('click', e => { const b = e.target.closest('button'); if (!b) return; state.fam = b.dataset.fam; store.set('fam', state.fam); render(); });
  $('invs').addEventListener('click', e => { const b = e.target.closest('button'); if (!b) return; state.inv = +b.dataset.inv; store.set('inv', state.inv); render(); });

  function keyboardSVG(keys, range, hands, label){
    const W = 22, H = 92, BW = 13, BH = 58;
    const map = new Map(keys.map(k => [k.abs, k]));
    const whites = [], blacks = [];
    let wi = 0;
    for (let a = range.start; a <= range.end; a++) {
      if (isBlack(a)) blacks.push({ a, x: wi * W - BW / 2 }); else { whites.push({ a, x: wi * W }); wi++; }
    }
    const width = wi * W + 1, height = H + (hands ? 18 : 0) + 1;
    let s = `<svg class="kb" viewBox="-0.5 -0.5 ${width} ${height}" role="img" aria-label="${label}">`;
    for (const k of whites) {
      const p = map.get(k.a);
      s += `<rect class="w${p ? ' r-' + p.role : ''}" x="${k.x}" y="0" width="${W}" height="${H}" rx="2"/>`;
    }
    for (const k of blacks) {
      const p = map.get(k.a);
      s += `<rect class="b${p ? ' r-' + p.role : ''}" x="${k.x}" y="0" width="${BW}" height="${BH}" rx="1.5"/>`;
    }
    for (const k of whites) { const p = map.get(k.a); if (p) s += `<text class="lab r-${p.role}" x="${k.x + W / 2}" y="${H - 9}" font-size="10">${p.name}</text>`; }
    for (const k of blacks) { const p = map.get(k.a); if (p) s += `<text class="lab bl r-${p.role}" x="${k.x + BW / 2}" y="${BH - 7}" font-size="8">${p.name}</text>`; }
    if (hands) {
      for (const k of whites.concat(blacks)) {
        const p = map.get(k.a);
        if (p && p.hand) { const cx = isBlack(k.a) ? k.x + BW / 2 : k.x + W / 2; s += `<text class="hand ${p.hand}" x="${cx}" y="${H + 14}">${p.hand}</text>`; }
      }
    }
    return s + '</svg>';
  }

  function cardHTML(t){
    const tones = chordTones(state.root, t.id);
    const rootName = ROOTS[state.root].name;
    const full = rootName + (t.sym ? ' ' + t.sym : '');
    const formula = tones.map(x => x.role === 'ext' ? `<b>${x.label}</b>` : x.label).join(' ');
    const notes = tones.map((x, i) => i === 0 ? `<span class="rt">${x.name}</span>` : x.name).join(' ');
    const vo = voiceInversion(state.root, t.id, state.inv);
    let cap = INV_NAMES[state.inv];
    let note = '';
    if (state.inv > 0) {
      if (vo.applied) cap += ` <span class="mono">bass ${vo.bass.label}</span>`;
      else { cap = INV_NAMES[0]; note = `<p class="note">No ${INV_NEED[state.inv]} in this chord, so root position is shown.</p>`; }
    }
    let html = `<article class="card" tabindex="0" role="button" aria-label="Play ${full}" data-type="${t.id}">
      <div class="card-head"><h3 class="cname"><span class="cr">${rootName}</span>${t.sym ? ' <span class="cs">' + t.sym + '</span>' : ''}</h3><span class="tname">${t.name}</span></div>
      <p class="formula">${formula}</p>
      <p class="notes">${notes}</p>
      <p class="use">${t.use}</p>
      <figure><figcaption>${cap}</figcaption>${keyboardSVG(vo.keys, vo.range, false, cap.replace(/<[^>]+>/g, '') + ': ' + vo.keys.map(k => k.name).join(' '))}</figure>${note}`;
    if (TWO_HAND_FAMS.includes(t.fam)) {
      const th = twoHandVoicing(state.root, t.id);
      const lhL = th.lh.map(k => k.label).join(' '), rhL = th.rh.map(k => k.label).join(' ');
      const om = th.omitted.length ? `, ${th.omitted.map(k => k.label).join(' ')} left out` : '';
      html += `<figure><figcaption>Two-hand voicing <span class="mono">LH ${lhL} · RH ${rhL}${om}</span></figcaption>${keyboardSVG(th.keys, th.range, true, 'Two-hand voicing. Left hand ' + th.lh.map(k => k.name).join(' ') + '. Right hand ' + th.rh.map(k => k.name).join(' '))}</figure>`;
    }
    const fg = fingering(t.id);
    const fgText = fg.lh.length ? `LH <span class="mono">${fg.lhF}</span> on ${fg.lh.map(s => tones.find(x => x.semis === s).label).join(' ')}, RH <span class="mono">${fg.rhF}</span> on the rest` : `RH <span class="mono">${fg.rhF}</span>`;
    html += `<p class="finger"><span class="k">Fingering, root position</span>${fgText}</p></article>`;
    return html;
  }

  function render(){
    document.querySelectorAll('#roots button').forEach(b => b.setAttribute('aria-pressed', String(+b.dataset.root === state.root)));
    document.querySelectorAll('#fams button').forEach(b => b.setAttribute('aria-pressed', String(b.dataset.fam === state.fam)));
    document.querySelectorAll('#invs button').forEach(b => b.setAttribute('aria-pressed', String(+b.dataset.inv === state.inv)));
    const fams = state.fam === 'all' ? FAMILIES : FAMILIES.filter(x => x.id === state.fam);
    $('out').innerHTML = fams.map(fm => {
      const list = TYPES.filter(t => t.fam === fm.id);
      return `<section class="fam" aria-labelledby="h-${fm.id}"><h2 id="h-${fm.id}">${fm.label} <small>${list.length} chords</small></h2>
        <div class="grid${TWO_HAND_FAMS.includes(fm.id) ? ' wide' : ''}">${list.map(cardHTML).join('')}</div></section>`;
    }).join('');
  }

  // Sound, started only from a user gesture
  let ctx = null;
  function play(typeId, card){
    try {
      if (!ctx) { const AC = window.AudioContext || window.webkitAudioContext; if (!AC) return; ctx = new AC(); }
      if (ctx.state === 'suspended') ctx.resume();
      const vo = voiceInversion(state.root, typeId, state.inv);
      const now = ctx.currentTime;
      const master = ctx.createGain(); master.gain.value = 0.5 / vo.keys.length; master.connect(ctx.destination);
      vo.keys.forEach((k, i) => {
        const o = ctx.createOscillator(), g = ctx.createGain();
        o.type = 'triangle';
        o.frequency.value = 440 * Math.pow(2, (60 + k.abs - 69) / 12);
        const t0 = now + i * 0.025;
        g.gain.setValueAtTime(0, t0);
        g.gain.linearRampToValueAtTime(1, t0 + 0.012);
        g.gain.exponentialRampToValueAtTime(0.001, t0 + 1.6);
        o.connect(g); g.connect(master); o.start(t0); o.stop(t0 + 1.7);
      });
      if (card) { card.classList.add('playing'); setTimeout(() => card.classList.remove('playing'), 600); }
    } catch (e) {}
  }
  $('out').addEventListener('click', e => { const c = e.target.closest('.card'); if (c) play(c.dataset.type, c); });
  $('out').addEventListener('keydown', e => {
    if (e.key !== 'Enter' && e.key !== ' ') return;
    const c = e.target.closest('.card'); if (!c || e.target !== c) return;
    e.preventDefault(); play(c.dataset.type, c);
  });

  render();
})();
