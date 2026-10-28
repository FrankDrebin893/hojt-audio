#!/usr/bin/env node
// Checks every fingering of the Guitar Chord Atlas (site/resources/guitar-chords/atlas.js) in all
// 12 keys: each sounding note belongs to the chord, every defining tone is there, the root is
// there, the frets are on the neck, and the hand can play it (four fingers, a barre only
// over strings that are pressed). Exits non-zero and says which shape if one fails.
//   node scripts/check-guitar-chords.js
'use strict';
const fs = require('fs');
const path = require('path');

const src = fs.readFileSync(path.join(__dirname, '..', 'site/resources/guitar-chords/atlas.js'), 'utf8');
const data = src.slice(src.indexOf('// @@DATA_START'), src.indexOf('// @@DATA_END'));
const A = new Function(data + '\nreturn { OPEN_PC, TYPES, FAMILIES, shapesFor, degreesOf };')();

let shapes = 0;
const errors = [];
function fail(msg) { if (errors.length < 60) errors.push(msg); }

function playable(shape) {
  const notes = [];
  shape.frets.forEach((f, s) => { if (f !== null && f > 0) notes.push({ s, f, finger: shape.fingers[s] }); });
  if (!notes.length) return null;
  if (notes.some((n) => !(n.finger >= 1 && n.finger <= 4))) return 'a pressed string has no finger 1-4';
  const byFinger = {};
  notes.forEach((n) => { (byFinger[n.finger] = byFinger[n.finger] || []).push(n); });
  for (const k in byFinger) {
    const ns = byFinger[k];
    const fr = new Set(ns.map((n) => n.f));
    if (fr.size > 1) return 'finger ' + k + ' on two frets';
    const f = ns[0].f;
    const lo = Math.min(...ns.map((n) => n.s)), hi = Math.max(...ns.map((n) => n.s));
    for (let s = lo; s <= hi; s++) {
      const x = shape.frets[s];
      if (x === null || x < f) return 'finger ' + k + ' lies over a string that is muted or lower';
    }
  }
  // fingers go up the neck with the frets
  const ks = Object.keys(byFinger).map(Number).sort((a, b) => a - b);
  for (let i = 1; i < ks.length; i++) if (byFinger[ks[i]][0].f < byFinger[ks[i - 1]][0].f) return 'fingers cross';
  const fs2 = notes.map((n) => n.f);
  if (Math.max(...fs2) - Math.min(...fs2) > 3) return 'wider than four frets';
  if (shape.barre) {
    const b = shape.barre;
    for (let s = b.from; s <= b.to; s++) {
      const x = shape.frets[s];
      if (x === null || x < b.fret) return 'barre over a muted or lower string';
    }
    if (!notes.some((n) => n.finger === b.finger && n.f === b.fret)) return 'barre finger not on its fret';
  }
  return null;
}

A.TYPES.forEach((t) => {
  const degs = A.degreesOf(t);
  // The natural 5th may be left out of a chord of four tones or more, and the 9th out of a 13th
  const has13 = degs.some((d) => d.n === 13);
  const need = degs.filter((d) => !d.optional && !(d.n === 5 && d.acc === 0 && t.iv.length >= 4) && !(has13 && d.n === 9 && d.acc === 0));
  const allowed = new Set(degs.map((d) => d.semis % 12));
  for (let root = 0; root < 12; root++) {
    A.shapesFor(t.id, root, true).forEach((sh, i) => {
      shapes++;
      const where = t.id + ' root ' + root + ' shape ' + i + ' (' + sh.label + ') ' + sh.frets.map((f) => f === null ? 'x' : f).join(' ');
      const pcs = new Set();
      sh.frets.forEach((f, s) => {
        if (f === null) return;
        if (f < 0 || f > 17) fail(where + ': fret ' + f + ' off the neck');
        const rel = ((A.OPEN_PC[s] + f - root) % 12 + 12) % 12;
        if (!allowed.has(rel)) fail(where + ': string ' + s + ' sounds ' + rel + ' semitones above the root, not in the chord');
        pcs.add(rel);
      });
      need.forEach((d) => { if (!pcs.has(d.semis % 12)) fail(where + ': missing ' + d.label); });
      if (!pcs.has(0)) fail(where + ': no root');
      if (!sh.bass && !sh.open) {
        const lowest = sh.frets.findIndex((f) => f !== null);
        if (((A.OPEN_PC[lowest] + sh.frets[lowest] - root) % 12 + 12) % 12 !== 0) fail(where + ': labelled as a root shape but the bass is not the root');
      }
      const sounding = sh.frets.filter((f) => f !== null).length;
      if (sounding < 2) fail(where + ': fewer than two strings');
      if (sh.bass) {
        const lowest = sh.frets.findIndex((f) => f !== null);
        const rel = ((A.OPEN_PC[lowest] + sh.frets[lowest] - root) % 12 + 12) % 12;
        const d = degs.find((x) => x.label.replace(/^bb/, '♭♭').replace(/^b/, '♭').replace(/^#/, '♯') === sh.bass);
        if (!d) fail(where + ': label says ' + sh.bass + ' in bass, which is not in the chord');
        else if (rel !== d.semis % 12) fail(where + ': bass is ' + rel + ' semitones above the root, label says ' + sh.bass);
      }
      const p = playable(sh);
      if (p) fail(where + ': ' + p);
    });
  }
});

if (errors.length) { console.error(errors.join('\n')); console.error(errors.length + ' problem(s)'); process.exit(1); }
console.log('guitar chords: ' + shapes + ' fingerings in ' + A.TYPES.length + ' chord types x 12 keys, all fine');
