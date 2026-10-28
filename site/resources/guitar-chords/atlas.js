// @@DATA_START
var OPEN_PC = [4, 9, 2, 7, 11, 4]; // low E -> high E
var STRING_NAMES = ['E', 'A', 'D', 'G', 'B', 'E'];
var ROOT_STRING = { E: 0, A: 1, D: 2 };
var KIND_LABEL = { E: '6th-string root', A: '5th-string root', D: '4th-string root', O: 'Open' };

var FAMILIES = [
  { id: 'triads', name: 'Triads & power' },
  { id: 'sixths', name: 'Sixths & adds' },
  { id: 'sevenths', name: 'Sevenths' },
  { id: 'extended', name: 'Extended' },
  { id: 'altered', name: 'Altered' }
];

// iv = interval formula in semitones (spec); f = degree labels in the same order; opt = optional extra degree
var TYPES = [
  { id: 'maj', sym: '', name: 'Major', fam: 'triads', iv: [0,4,7], f: '1 3 5', use: 'Bright and stable; home base' },
  { id: 'min', sym: 'm', name: 'Minor', fam: 'triads', iv: [0,3,7], f: '1 b3 5', use: 'Darker, serious or sad; stable' },
  { id: '5', sym: '5', name: 'Power chord', fam: 'triads', iv: [0,7], f: '1 5', use: 'Neutral and heavy; great with distortion' },
  { id: 'sus2', sym: 'sus2', name: 'Suspended 2nd', fam: 'triads', iv: [0,2,7], f: '1 2 5', use: 'Open and airy; no third' },
  { id: 'sus4', sym: 'sus4', name: 'Suspended 4th', fam: 'triads', iv: [0,5,7], f: '1 4 5', use: 'Hanging tension; resolves to major' },
  { id: 'aug', sym: 'aug', name: 'Augmented', fam: 'triads', iv: [0,4,8], f: '1 3 #5', use: 'Dreamy and unsettled; pushes upward' },
  { id: 'dim', sym: 'dim', name: 'Diminished', fam: 'triads', iv: [0,3,6], f: '1 b3 b5', use: 'Tense and unstable; passing chord' },

  { id: '6', sym: '6', name: 'Major 6th', fam: 'sixths', iv: [0,4,7,9], f: '1 3 5 6', use: 'Sweet vintage major; swing and pop' },
  { id: 'm6', sym: 'm6', name: 'Minor 6th', fam: 'sixths', iv: [0,3,7,9], f: '1 b3 5 6', use: 'Bittersweet minor; jazz and bossa' },
  { id: '69', sym: '6/9', name: 'Six-nine', fam: 'sixths', iv: [0,4,7,9,14], f: '1 3 5 6 9', use: 'Lush open major; jazz endings' },
  { id: 'add9', sym: 'add9', name: 'Added 9th', fam: 'sixths', iv: [0,4,7,14], f: '1 3 5 9', use: 'Shimmering major; pop and folk' },
  { id: 'madd9', sym: 'm(add9)', name: 'Minor added 9th', fam: 'sixths', iv: [0,3,7,14], f: '1 b3 5 9', use: 'Moody, wistful minor color' },

  { id: '7', sym: '7', name: 'Dominant 7th', fam: 'sevenths', iv: [0,4,7,10], f: '1 3 5 b7', use: 'Bluesy dominant; wants to resolve' },
  { id: 'maj7', sym: 'maj7', name: 'Major 7th', fam: 'sevenths', iv: [0,4,7,11], f: '1 3 5 7', use: 'Soft, dreamy, jazzy major' },
  { id: 'm7', sym: 'm7', name: 'Minor 7th', fam: 'sevenths', iv: [0,3,7,10], f: '1 b3 5 b7', use: 'Mellow minor; soul, funk, jazz' },
  { id: 'mmaj7', sym: 'm(maj7)', name: 'Minor-major 7th', fam: 'sevenths', iv: [0,3,7,11], f: '1 b3 5 7', use: 'Dark and cinematic; spy-film minor' },
  { id: 'm7b5', sym: 'm7♭5', name: 'Half-diminished', fam: 'sevenths', iv: [0,3,6,10], f: '1 b3 b5 b7', use: 'The ii chord in minor keys' },
  { id: 'dim7', sym: 'dim7', name: 'Diminished 7th', fam: 'sevenths', iv: [0,3,6,9], f: '1 b3 b5 bb7', use: 'Symmetric tension; slides anywhere' },
  { id: '7sus4', sym: '7sus4', name: 'Dominant 7th sus4', fam: 'sevenths', iv: [0,5,7,10], f: '1 4 5 b7', use: 'Dominant with suspense; gospel and funk' },
  { id: '7sharp5', sym: '7♯5', name: 'Augmented 7th', fam: 'sevenths', iv: [0,4,8,10], f: '1 3 #5 b7', use: 'Tense dominant; leans hard into resolution' },
  { id: '7b5', sym: '7♭5', name: 'Dominant 7th flat 5', fam: 'sevenths', iv: [0,4,6,10], f: '1 3 b5 b7', use: 'Edgy whole-tone dominant' },

  { id: '9', sym: '9', name: 'Dominant 9th', fam: 'extended', iv: [0,4,7,10,14], f: '1 3 5 b7 9', use: 'Full, funky dominant; blues and funk' },
  { id: 'maj9', sym: 'maj9', name: 'Major 9th', fam: 'extended', iv: [0,4,7,11,14], f: '1 3 5 7 9', use: 'Warm, lush major; neo-soul and jazz' },
  { id: 'm9', sym: 'm9', name: 'Minor 9th', fam: 'extended', iv: [0,3,7,10,14], f: '1 b3 5 b7 9', use: 'Smooth, rich minor; R&B and jazz' },
  { id: '11', sym: '11', name: 'Dominant 11th', fam: 'extended', iv: [0,7,10,14,17], f: '1 5 b7 9 11', use: 'Suspended dominant; gospel and fusion' },
  { id: 'm11', sym: 'm11', name: 'Minor 11th', fam: 'extended', iv: [0,3,7,10,14,17], f: '1 b3 5 b7 9 11', use: 'Wide open minor; modal jazz' },
  { id: '13', sym: '13', name: 'Dominant 13th', fam: 'extended', iv: [0,4,10,14,21], f: '1 3 b7 9 13', opt: '5', use: 'Rich dominant; jazz and big band' },
  { id: 'm13', sym: 'm13', name: 'Minor 13th', fam: 'extended', iv: [0,3,10,14,21], f: '1 b3 b7 9 13', use: 'Dorian minor color; smooth jazz' },
  { id: 'maj13', sym: 'maj13', name: 'Major 13th', fam: 'extended', iv: [0,4,11,14,21], f: '1 3 7 9 13', use: 'Airy, sophisticated major; ballad endings' },

  { id: '7b9', sym: '7♭9', name: 'Dominant 7th flat 9', fam: 'altered', iv: [0,4,7,10,13], f: '1 3 5 b7 b9', use: 'Dark dominant; resolves to minor' },
  { id: '7sharp9', sym: '7♯9', name: 'Dominant 7th sharp 9', fam: 'altered', iv: [0,4,7,10,15], f: '1 3 5 b7 #9', use: 'Gritty blues-rock dominant; the Hendrix chord' },
  { id: 'maj7sharp11', sym: 'maj7♯11', name: 'Lydian major 7th', fam: 'altered', iv: [0,4,7,11,18], f: '1 3 5 7 #11', use: 'Bright Lydian color; floating, modern' },
  { id: '7sharp11', sym: '7♯11', name: 'Lydian dominant', fam: 'altered', iv: [0,4,7,10,18], f: '1 3 5 b7 #11', use: 'Bright, jazzy dominant tension' },
  { id: '7alt', sym: '7alt (7♯5♯9)', name: 'Altered dominant', fam: 'altered', iv: [0,4,8,10,15], f: '1 3 #5 b7 #9', use: 'Maximum dominant tension; jazz turnarounds' }
];

// Movable shapes. frets: relative to the root fret, low E -> high E, x = muted.
// fingers: 1-4 per string (x for muted). barre: "relFret:fromString-toString:finger" (string 0 = low E).
var SHAPES = {
  'maj':   [ ['E','0 2 2 1 0 0','1 3 4 2 1 1','0:0-5:1'], ['A','x 0 2 2 2 0','x 1 2 3 4 1','0:1-5:1'] ],
  'min':   [ ['E','0 2 2 0 0 0','1 3 4 1 1 1','0:0-5:1'], ['A','x 0 2 2 1 0','x 1 3 4 2 1','0:1-5:1'] ],
  '5':     [ ['E','0 2 2 x x x','1 3 4 x x x'], ['A','x 0 2 2 x x','x 1 3 4 x x'] ],
  'sus2':  [ ['E','0 2 2 -1 x x','2 3 4 1 x x'], ['A','x 0 2 2 0 0','x 1 3 4 1 1','0:1-5:1'] ],
  'sus4':  [ ['E','0 2 2 2 0 0','1 2 3 4 1 1','0:0-5:1'], ['A','x 0 2 2 3 0','x 1 2 3 4 1','0:1-5:1'] ],
  'aug':   [ ['E','0 x 2 1 1 x','1 x 4 2 3 x'], ['A','x 0 3 2 2 x','x 1 4 2 3 x'] ],
  'dim':   [ ['E','0 1 2 0 x x','1 2 3 1 x x','0:0-3:1'], ['A','x 0 1 2 1 x','x 1 2 4 3 x'] ],

  '6':     [ ['E','0 x -1 1 0 x','2 x 1 4 3 x'], ['A','x 0 2 -1 2 x','x 2 3 1 4 x'], ['D','x x 0 2 0 2','x x 1 3 2 4'] ],
  'm6':    [ ['E','0 x -1 0 0 x','2 x 1 3 4 x'], ['A','x 0 2 -1 1 x','x 2 4 1 3 x'], ['D','x x 0 2 0 1','x x 1 3 1 2','0:2-4:1'] ],
  '69':    [ ['E','0 -1 -1 -1 0 0','2 1 1 1 3 4','-1:1-3:1'], ['A','x 0 -1 -1 0 0','x 2 1 1 3 4','-1:2-3:1'] ],
  'add9':  [ ['E','0 -1 -3 -1 x x','4 2 1 3 x x'], ['A','x 0 -1 -3 0 x','x 3 2 1 4 x'] ],
  'madd9': [ ['E','0 -2 -3 -1 x x','4 2 1 3 x x'], ['A','x 0 -2 -3 0 x','x 3 2 1 4 x'] ],

  '7':     [ ['E','0 2 0 1 0 0','1 3 1 2 1 1','0:0-5:1'], ['A','x 0 2 0 2 0','x 1 3 1 4 1','0:1-5:1'], ['D','x x 0 2 1 2','x x 1 3 2 4'] ],
  'maj7':  [ ['E','0 x 1 1 0 x','1 x 3 4 2 x'], ['A','x 0 2 1 2 0','x 1 3 2 4 1','0:1-5:1'], ['D','x x 0 2 2 2','x x 1 3 3 3','2:3-5:3'] ],
  'm7':    [ ['E','0 2 0 0 0 0','1 3 1 1 1 1','0:0-5:1'], ['A','x 0 2 0 1 0','x 1 3 1 2 1','0:1-5:1'], ['D','x x 0 2 1 1','x x 1 3 2 2','1:4-5:2'] ],
  'mmaj7': [ ['E','0 2 1 0 0 0','1 3 2 1 1 1','0:0-5:1'], ['A','x 0 2 1 1 0','x 1 4 2 3 1','0:1-5:1'] ],
  'm7b5':  [ ['E','0 x 0 0 -1 x','2 x 3 4 1 x'], ['A','x 0 1 0 1 x','x 1 3 2 4 x'], ['D','x x 0 1 1 1','x x 1 2 2 2','1:3-5:2'] ],
  'dim7':  [ ['E','0 x -1 0 -1 x','2 x 1 3 1 x','-1:2-4:1'], ['A','x 0 1 -1 1 x','x 2 3 1 4 x'], ['D','x x 0 1 0 1','x x 1 3 2 4'] ],
  '7sus4': [ ['E','0 2 0 2 0 0','1 3 1 4 1 1','0:0-5:1'], ['A','x 0 2 0 3 0','x 1 3 1 4 1','0:1-5:1'], ['D','x x 0 2 1 3','x x 1 3 2 4'] ],
  '7sharp5': [ ['E','0 x 0 1 1 x','1 x 2 3 4 x'], ['A','x 0 x 0 2 1','x 1 x 2 4 3'] ],
  '7b5':   [ ['E','0 1 0 1 x x','1 3 2 4 x x'], ['A','x 0 1 0 2 x','x 1 3 2 4 x'] ],

  '9':     [ ['E','0 -1 0 -1 0 x','2 1 3 1 4 x','-1:1-3:1'], ['A','x 0 -1 0 0 0','x 2 1 3 3 3','0:3-5:3'], ['D','x x 0 -1 1 0','x x 2 1 4 3'] ],
  'maj9':  [ ['E','0 -1 1 -1 x x','2 1 3 1 x x','-1:1-3:1'], ['A','x 0 -1 1 0 x','x 2 1 4 3 x'], ['D','x x 0 -1 2 0','x x 2 1 4 3'] ],
  'm9':    [ ['E','0 -2 0 -1 x x','3 1 4 2 x x'], ['A','x 0 -2 0 0 0','x 2 1 3 3 3','0:3-5:3'], ['D','x x 0 -2 1 0','x x 2 1 4 3'] ],
  '11':    [ ['E','0 x 0 -1 -2 x','3 x 4 2 1 x'], ['A','x 0 0 0 0 0','x 1 1 1 1 1','0:1-5:1'] ],
  'm11':   [ ['E','0 0 0 0 x 2','1 1 1 1 x 3','0:0-3:1'], ['A','x 0 -2 0 0 -2','x 2 1 3 4 1','-2:2-5:1'] ],
  '13':    [ ['E','0 x 0 1 2 x','1 x 2 3 4 x'], ['A','x 0 -1 0 0 2','x 2 1 3 3 4','0:3-4:3'] ],
  'm13':   [ ['E','0 x 0 0 2 x','1 x 2 3 4 x'], ['A','x 0 x 0 1 2','x 1 x 2 3 4'] ],
  'maj13': [ ['E','0 x 1 1 2 x','1 x 2 3 4 x'], ['A','x 0 -1 1 x 2','x 2 1 3 x 4'] ],

  '7b9':   [ ['E','0 -1 0 -2 x x','3 2 4 1 x x'], ['A','x 0 -1 0 -1 x','x 2 1 3 1 x','-1:2-4:1'] ],
  '7sharp9': [ ['E','0 -1 0 0 x x','2 1 3 4 x x'], ['A','x 0 -1 0 1 x','x 2 1 3 4 x'] ],
  'maj7sharp11': [ ['E','0 x 1 1 -1 x','2 x 3 4 1 x'], ['A','x 0 1 1 2 x','x 1 2 3 4 x'] ],
  '7sharp11': [ ['E','0 x 0 1 -1 x','2 x 3 4 1 x'], ['A','x 0 1 0 2 x','x 1 3 2 4 x'] ],
  '7alt':  [ ['E','0 x 0 1 1 3','1 x 2 3 3 4','1:3-4:3'], ['A','x 0 -1 0 1 1','x 2 1 3 4 4','1:4-5:4'] ]
};

// Open-position (CAGED) shapes in absolute frets, keyed by root pitch class.
var OPEN_SHAPES = {
  0: { // C
    'maj': ['x 3 2 0 1 0','x 3 2 0 1 0'], '7': ['x 3 2 3 1 0','x 3 2 4 1 0'], 'maj7': ['x 3 2 0 0 0','x 3 2 0 0 0'],
    'sus2': ['x 3 0 0 1 3','x 3 0 0 1 4'], 'sus4': ['x 3 3 0 1 1','x 3 4 0 1 1','1:4-5:1'],
    'add9': ['x 3 2 0 3 0','x 2 1 0 3 0'], '6': ['x 3 2 2 1 0','x 4 2 3 1 0']
  },
  9: { // A
    'maj': ['x 0 2 2 2 0','x 0 1 2 3 0'], 'min': ['x 0 2 2 1 0','x 0 2 3 1 0'], '7': ['x 0 2 0 2 0','x 0 2 0 3 0'],
    'maj7': ['x 0 2 1 2 0','x 0 2 1 3 0'], 'm7': ['x 0 2 0 1 0','x 0 2 0 1 0'], 'sus2': ['x 0 2 2 0 0','x 0 1 2 0 0'],
    'sus4': ['x 0 2 2 3 0','x 0 1 2 3 0'], 'add9': ['x 0 2 4 2 0','x 0 1 4 2 0'], '6': ['x 0 2 2 2 2','x 0 1 1 1 1','2:2-5:1']
  },
  7: { // G
    'maj': ['3 2 0 0 0 3','2 1 0 0 0 3'], '7': ['3 2 0 0 0 1','3 2 0 0 0 1'], 'maj7': ['3 2 0 0 0 2','3 2 0 0 0 1'],
    'sus2': ['3 0 0 2 3 3','2 0 0 1 3 4'], 'sus4': ['3 3 0 0 1 3','2 3 0 0 1 4'], 'add9': ['3 2 0 2 0 3','3 1 0 2 0 4'],
    '6': ['3 2 0 0 0 0','2 1 0 0 0 0']
  },
  4: { // E
    'maj': ['0 2 2 1 0 0','0 2 3 1 0 0'], 'min': ['0 2 2 0 0 0','0 2 3 0 0 0'], '7': ['0 2 0 1 0 0','0 2 0 1 0 0'],
    'maj7': ['0 2 1 1 0 0','0 3 1 2 0 0'], 'm7': ['0 2 0 0 0 0','0 2 0 0 0 0'], 'sus2': ['0 2 4 4 0 0','0 1 3 4 0 0'],
    'sus4': ['0 2 2 2 0 0','0 2 3 4 0 0'], 'add9': ['0 2 4 1 0 0','0 2 4 1 0 0'], '6': ['0 2 2 1 2 0','0 2 3 1 4 0']
  },
  2: { // D
    'maj': ['x x 0 2 3 2','x x 0 1 3 2'], 'min': ['x x 0 2 3 1','x x 0 2 3 1'], '7': ['x x 0 2 1 2','x x 0 2 1 3'],
    'maj7': ['x x 0 2 2 2','x x 0 1 1 1','2:3-5:1'], 'm7': ['x x 0 2 1 1','x x 0 2 1 1','1:4-5:1'], 'sus2': ['x x 0 2 3 0','x x 0 1 3 0'],
    'sus4': ['x x 0 2 3 3','x x 0 1 3 4'], 'add9': ['x 5 4 2 3 0','x 4 3 1 2 0'], '6': ['x x 0 2 0 2','x x 0 2 0 3']
  }
};

function parseList(s) { return s.trim().split(/\s+/).map(function (t) { return t === 'x' ? null : parseInt(t, 10); }); }
function parseBarre(s) {
  if (!s) return null;
  var m = /^(-?\d+):(\d)-(\d):(\d)$/.exec(s);
  return { fret: +m[1], from: +m[2], to: +m[3], finger: +m[4] };
}

// Place a movable shape for a root pitch class: lowest position where every fret >= 0.
function placeShape(spec, rootPc) {
  var kind = spec[0], rel = parseList(spec[1]), fing = parseList(spec[2]), barre = parseBarre(spec[3]);
  var rs = ROOT_STRING[kind];
  var base = ((rootPc - OPEN_PC[rs]) % 12 + 12) % 12;
  var minRel = Math.min.apply(null, rel.filter(function (v) { return v !== null; }));
  if (base + minRel < 0) base += 12;
  var frets = rel.map(function (v) { return v === null ? null : base + v; });
  var fingers = fing.slice();
  // Strings that land on the nut become open strings: drop their finger and renumber if needed.
  var usedOpen = false;
  frets.forEach(function (f, i) { if (f === 0) { fingers[i] = 0; usedOpen = true; } });
  var b = barre ? { fret: base + barre.fret, from: barre.from, to: barre.to, finger: barre.finger } : null;
  if (b && b.fret === 0) b = null;
  if (usedOpen) {
    var used = fingers.filter(function (x) { return x; });
    if (used.length && used.indexOf(1) === -1) {
      fingers = fingers.map(function (x) { return x ? x - 1 : x; });
      if (b) b.finger -= 1;
    }
  }
  return { kind: kind, label: KIND_LABEL[kind], frets: frets, fingers: fingers, barre: b, rootString: rs, base: base, rel: rel };
}
function openShape(spec) {
  var frets = parseList(spec[0]);
  var rs = frets.findIndex(function (f) { return f !== null; });
  return { kind: 'O', label: KIND_LABEL.O, frets: frets, fingers: parseList(spec[1]), barre: parseBarre(spec[2]), rootString: rs, open: true };
}
function shapesFor(typeId, rootPc) {
  var out = (SHAPES[typeId] || []).map(function (s) { return placeShape(s, rootPc); });
  var o = OPEN_SHAPES[rootPc] && OPEN_SHAPES[rootPc][typeId];
  if (o) out.push(openShape(o));
  return out;
}

// Degree labels -> semitones and letter steps
var DEG_BASE = { 1: 0, 2: 2, 3: 4, 4: 5, 5: 7, 6: 9, 7: 11, 9: 14, 11: 17, 13: 21 };
function parseDeg(lbl) {
  var m = /^(bb|b|#)?(\d+)$/.exec(lbl);
  var acc = m[1] === 'bb' ? -2 : m[1] === 'b' ? -1 : m[1] === '#' ? 1 : 0;
  var n = +m[2];
  return { label: lbl, n: n, semis: DEG_BASE[n] + acc, step: (n - 1) % 7, acc: acc };
}
function degreesOf(type) {
  var d = type.f.split(' ').map(parseDeg);
  if (type.opt) d = d.concat(type.opt.split(' ').map(function (l) { var p = parseDeg(l); p.optional = true; return p; }));
  return d;
}
// @@DATA_END

(function () {
  'use strict';
  var NS = 'http://www.w3.org/2000/svg';
  var LETTERS = 'CDEFGAB';
  var LETTER_PC = [0, 2, 4, 5, 7, 9, 11];
  // Root options: [letterIndex, accidental]
  var ROOTS = [
    { pc: 0, opts: [[0, 0]] },
    { pc: 1, opts: [[0, 1], [1, -1]] },
    { pc: 2, opts: [[1, 0]] },
    { pc: 3, opts: [[2, -1]] },
    { pc: 4, opts: [[2, 0]] },
    { pc: 5, opts: [[3, 0]] },
    { pc: 6, opts: [[3, 1], [4, -1]] },
    { pc: 7, opts: [[4, 0]] },
    { pc: 8, opts: [[5, -1]] },
    { pc: 9, opts: [[5, 0]] },
    { pc: 10, opts: [[6, -1]] },
    { pc: 11, opts: [[6, 0]] }
  ];
  function accStr(a) {
    if (a === 0) return '';
    if (a > 0) return a === 1 ? '♯' : a === 2 ? '𝄪' : '♯'.repeat(a);
    return '♭'.repeat(-a);
  }
  function noteName(letterIdx, acc) { return LETTERS[letterIdx] + accStr(acc); }
  function spellChord(rootOpt, rootPc, degs) {
    var total = 0;
    var names = degs.map(function (d) {
      var li = (rootOpt[0] + d.step) % 7;
      var pc = (rootPc + d.semis) % 12;
      var acc = ((pc - LETTER_PC[li]) % 12 + 18) % 12 - 6;
      total += Math.abs(acc);
      return noteName(li, acc);
    });
    return { names: names, rootName: noteName(rootOpt[0], rootOpt[1]), cost: total };
  }
  function bestSpelling(root, degs) {
    // One spelling per page so every card shares the same root name (sharp side for C♯/D♭ and F♯/G♭).
    return spellChord(root.opts[0], root.pc, degs);
  }
  function prettyDeg(lbl) { return lbl.replace(/^bb/, '♭♭').replace(/^b/, '♭').replace(/^#/, '♯'); }
  function degRole(d) {
    if (d.semis === 0) return 'root';
    if (d.n === 6 || d.n === 9 || d.n === 11 || d.n === 13) return 'color';
    if (d.n === 5 && d.acc !== 0) return 'color';
    return 'tone';
  }

  // ---------- storage ----------
  function load(k, def) { try { var v = localStorage.getItem(k); return v === null ? def : v; } catch (e) { return def; } }
  function save(k, v) { try { localStorage.setItem(k, v); } catch (e) { /* ignore */ } }

  var state = {
    root: parseInt(load('gca-root', '0'), 10),
    fam: load('gca-fam', 'all'),
    show: load('gca-show', 'fingers')
  };
  if (!(state.root >= 0 && state.root < 12)) state.root = 0;
  if (state.fam !== 'all' && !FAMILIES.some(function (f) { return f.id === state.fam; })) state.fam = 'all';
  if (['fingers', 'intervals', 'notes'].indexOf(state.show) === -1) state.show = 'fingers';

  // ---------- controls ----------
  var ROOT_LABELS = ['C', ['C♯', 'D♭'], 'D', 'E♭', 'E', 'F', ['F♯', 'G♭'], 'G', 'A♭', 'A', 'B♭', 'B'];
  var picker = document.getElementById('rootPicker');
  ROOT_LABELS.forEach(function (l, pc) {
    var b = document.createElement('button');
    b.type = 'button';
    b.dataset.pc = pc;
    if (Array.isArray(l)) {
      b.innerHTML = l[0] + '<span class="alt">' + l[1] + '</span>';
      b.setAttribute('aria-label', l[0] + ' or ' + l[1]);
    } else b.textContent = l;
    b.addEventListener('click', function () { state.root = pc; save('gca-root', String(pc)); syncControls(); render(); });
    picker.appendChild(b);
  });
  var chips = document.getElementById('famChips');
  [{ id: 'all', name: 'All' }].concat(FAMILIES).forEach(function (f) {
    var b = document.createElement('button');
    b.type = 'button'; b.className = 'chip'; b.dataset.fam = f.id; b.textContent = f.name;
    b.addEventListener('click', function () { state.fam = f.id; save('gca-fam', f.id); syncControls(); applyFilter(); });
    chips.appendChild(b);
  });
  var tog = document.getElementById('labelTog');
  [['fingers', 'Fingers'], ['intervals', 'Intervals'], ['notes', 'Notes']].forEach(function (o) {
    var b = document.createElement('button');
    b.type = 'button'; b.dataset.show = o[0]; b.textContent = o[1];
    b.addEventListener('click', function () { state.show = o[0]; save('gca-show', o[0]); syncControls(); render(); });
    tog.appendChild(b);
  });
  function syncControls() {
    picker.querySelectorAll('button').forEach(function (b) { b.setAttribute('aria-pressed', String(+b.dataset.pc === state.root)); });
    chips.querySelectorAll('button').forEach(function (b) { b.setAttribute('aria-pressed', String(b.dataset.fam === state.fam)); });
    tog.querySelectorAll('button').forEach(function (b) { b.setAttribute('aria-pressed', String(b.dataset.show === state.show)); });
  }

  // ---------- SVG diagram ----------
  function el(name, attrs, text) {
    var e = document.createElementNS(NS, name);
    for (var k in attrs) e.setAttribute(k, attrs[k]);
    if (text != null) e.textContent = text;
    return e;
  }
  var X0 = 24, SX = 15, Y0 = 22, SY = 22, NF = 5, R = 7.2;
  function diagram(shape, pcInfo) {
    var fretted = shape.frets.filter(function (f) { return f !== null && f > 0; });
    var maxF = fretted.length ? Math.max.apply(null, fretted) : 0;
    var minF = fretted.length ? Math.min.apply(null, fretted) : 1;
    var start = maxF <= NF ? 1 : minF;
    var W = X0 + SX * 5 + 14, H = Y0 + SY * NF + 22;
    var svg = el('svg', { viewBox: '0 0 ' + W + ' ' + H, width: W, height: H, role: 'img' });
    var tabTxt = shape.frets.map(function (f) { return f === null ? 'x' : String(f); }).join(' ');
    svg.setAttribute('aria-label', shape.label + ': ' + tabTxt.replace(/x/g, 'muted'));
    // frets
    for (var i = 0; i <= NF; i++) {
      var y = Y0 + i * SY;
      svg.appendChild(el('line', { x1: X0, x2: X0 + SX * 5, y1: y, y2: y, 'class': (i === 0 && start === 1) ? 'nut' : 'fret', 'stroke-width': (i === 0 && start === 1) ? 4 : 1 }));
    }
    for (var s = 0; s < 6; s++) {
      var x = X0 + s * SX;
      svg.appendChild(el('line', { x1: x, x2: x, y1: Y0, y2: Y0 + SY * NF, 'class': 'str', 'stroke-width': 1 }));
    }
    if (start > 1) svg.appendChild(el('text', { x: X0 - 8, y: Y0 + SY / 2 + 3.5, 'text-anchor': 'end', 'class': 'lbl' }, start + 'fr'));
    // barre
    if (shape.barre && shape.barre.fret >= start) {
      var by = Y0 + (shape.barre.fret - start + 0.5) * SY;
      var bx1 = X0 + shape.barre.from * SX - R, bx2 = X0 + shape.barre.to * SX + R;
      svg.appendChild(el('rect', { x: bx1, y: by - R, width: bx2 - bx1, height: 2 * R, rx: R, 'class': 'bar' }));
    }
    // dots / markers
    shape.frets.forEach(function (f, s) {
      var x = X0 + s * SX;
      if (f === null) {
        var m = 3.6, cy = Y0 - 11;
        svg.appendChild(el('path', { d: 'M' + (x - m) + ' ' + (cy - m) + 'L' + (x + m) + ' ' + (cy + m) + 'M' + (x + m) + ' ' + (cy - m) + 'L' + (x - m) + ' ' + (cy + m), 'class': 'mute', 'stroke-width': 1.6, fill: 'none' }));
        return;
      }
      var pc = (OPEN_PC[s] + f) % 12;
      var info = pcInfo[pc] || { role: 'tone', deg: '?', note: '?' };
      if (f === 0) {
        svg.appendChild(el('circle', { cx: x, cy: Y0 - 11, r: 4.4, 'class': 'o-' + info.role, 'stroke-width': 1.8 }));
        return;
      }
      var cy2 = Y0 + (f - start + 0.5) * SY;
      svg.appendChild(el('circle', { cx: x, cy: cy2, r: R, 'class': 'd-' + info.role }));
      var t = state.show === 'fingers' ? (shape.fingers[s] ? String(shape.fingers[s]) : '')
        : state.show === 'intervals' ? (info.deg === '1' ? 'R' : prettyDeg(info.deg)) : info.note;
      if (t) {
        var fs = t.length <= 1 ? 10 : t.length === 2 ? 8.5 : 7;
        svg.appendChild(el('text', { x: x, y: cy2 + fs * 0.36, 'text-anchor': 'middle', 'class': 'dt', 'font-size': fs }, t));
      }
    });
    svg.appendChild(el('text', { x: X0 + SX * 2.5, y: H - 4, 'text-anchor': 'middle', 'class': 'tab' }, tabTxt));
    var fig = document.createElement('figure');
    fig.className = 'dg';
    fig.appendChild(svg);
    var cap = document.createElement('figcaption');
    cap.textContent = shape.label;
    fig.appendChild(cap);
    return fig;
  }

  // ---------- page ----------
  var atlas = document.getElementById('atlas');
  function render() {
    var root = ROOTS[state.root];
    var frag = document.createDocumentFragment();
    FAMILIES.forEach(function (fam) {
      var sec = document.createElement('section');
      sec.className = 'fam'; sec.dataset.fam = fam.id;
      var types = TYPES.filter(function (t) { return t.fam === fam.id; });
      var h = document.createElement('h2');
      h.id = 'fam-' + fam.id;
      h.innerHTML = fam.name + ' <span>' + types.length + ' chords</span>';
      sec.setAttribute('aria-labelledby', h.id);
      sec.appendChild(h);
      var grid = document.createElement('div');
      grid.className = 'grid';
      types.forEach(function (t) {
        var degs = degreesOf(t);
        var sp = bestSpelling(root, degs);
        var pcInfo = {};
        degs.forEach(function (d, i) {
          var pc = (root.pc + d.semis) % 12;
          pcInfo[pc] = { role: degRole(d), deg: d.label, note: sp.names[i] };
        });
        var card = document.createElement('article');
        card.className = 'card';
        var h3 = document.createElement('h3');
        h3.textContent = sp.rootName + (t.sym ? ' ' + t.sym : '');
        card.appendChild(h3);
        var tn = document.createElement('div'); tn.className = 'tname'; tn.textContent = t.name;
        card.appendChild(tn);
        var dl = document.createElement('dl'); dl.className = 'meta';
        var fHtml = degs.map(function (d) {
          var p = prettyDeg(d.label);
          var r = degRole(d);
          var txt = d.optional ? '(' + p + ')' : p;
          return r === 'color' ? '<b>' + txt + '</b>' : r === 'root' ? '<span class="r">' + txt + '</span>' : txt;
        }).join(' ');
        var nHtml = degs.map(function (d, i) {
          var r = degRole(d), n = d.optional ? '(' + sp.names[i] + ')' : sp.names[i];
          return r === 'color' ? '<b>' + n + '</b>' : r === 'root' ? '<span class="r">' + n + '</span>' : n;
        }).join(' ');
        dl.innerHTML = '<dt>Formula</dt><dd>' + fHtml + '</dd><dt>Notes</dt><dd>' + nHtml + '</dd>';
        card.appendChild(dl);
        var use = document.createElement('p'); use.className = 'use'; use.textContent = t.use;
        card.appendChild(use);
        var dg = document.createElement('div'); dg.className = 'diagrams';
        shapesFor(t.id, root.pc).forEach(function (s) { dg.appendChild(diagram(s, pcInfo)); });
        card.appendChild(dg);
        grid.appendChild(card);
      });
      sec.appendChild(grid);
      frag.appendChild(sec);
    });
    atlas.replaceChildren(frag);
    applyFilter();
  }
  function applyFilter() {
    atlas.querySelectorAll('section.fam').forEach(function (s) {
      s.hidden = !(state.fam === 'all' || s.dataset.fam === state.fam);
    });
  }
  syncControls();
  render();
})();
