# Hardware display concepts

Five looks explored on 2026-10-19, all built on one idea: the window is an instrument's
front panel. The chrome becomes the panel material and the stage becomes the display.
Every mockup shows the same moment: G7/B held in C major, with four chords saved.

The live canvas, where these can be viewed and tweaked:
https://claude.ai/artifact/A1vkXJUVwwK3oD11wKC69h

| File | Concept | Status |
|---|---|---|
| `Scope.dc.html` | **E · Phosphor scope**: a curved CRT with a graticule, scanlines and green phosphor glow; the chord drawn as a live waveform trace; a wireframe keyboard; instrument knobs and tape-labelled memory buttons | Tried in the app 2026-10-19, replaced by K (git history: 30a6ba0, 08255c7) |
| `VFD.dc.html` | A · VFD workstation: smoked glass with cyan phosphor, an amber key centre, rubber buttons and a data wheel | Kept for later |
| `LCD.dc.html` | B · Backlit LCD with real keys: a yellow-green pixel LCD, soft keys under on-screen tabs, real ivory keys with an LED above each | Kept for later |
| `OLED.dc.html` | C · OLED module: pure black OLED in aluminium, geometric type, a ring dial and coloured encoders | Kept for later |
| `Rack.dc.html` | D · LED rack unit: a black rack panel, red LED dot matrix, an LED ring around the key knob, a level-meter keyboard | Kept for later |

Round 2 (2026-10-20), asked for in the spirit of B and C, all with real keys: ivory and
ebony keys that go down when held, with the display above them.

| File | Concept | Status |
|---|---|---|
| `FSTN.dc.html` | F · Blue FSTN workstation: a white-on-blue pixel LCD with soft keys under its tabs, faders, a jog dial and keypad, pitch and mod wheels, and a blue key light above each held key | Kept for later |
| `Stage.dc.html` | G · Walnut stage piano: walnut cheeks, a wide OLED strip across satin black, flat touch buttons with LEDs, a full-width keyboard | Kept for later |
| `Home.dc.html` | H · 80s home keyboard: a cream case with speaker grilles, an amber segment LCD with printed flags and a mini keyboard of held notes, a key-select dial in fifths, colourful round chord buttons | Kept for later |
| `Memory.dc.html` | I · Reflective LCD on aluminium: an unlit grey-green memory LCD with crisp ink, an orange save button, a tempo slider, quiet round buttons | Kept for later |
| `Synth.dc.html` | J · Colour OLED synth: wood cheeks, a colour OLED, LED-ring encoders, backlit progression pads | Kept for later |

Round 3 (2026-10-20): a scarlet-red colour scheme, still with real keys. The finish is a
generic red anodised or lacquered case; there are no brand names or logos.

| File | Concept | Status |
|---|---|---|
| `Scarlet.dc.html` | **K · Scarlet anodised**: brushed red aluminium, a white-on-black OLED window with soft keys, two large knobs with green-to-red level halos, black rubber buttons | **Chosen**: built into the app, halo knobs (volume, click) included |
| `Touch.dc.html` | L · Scarlet and black: a matte black panel between red anodised end plates, a colour touchscreen laid out in cards, a glowing red strip above the keys | Kept for later |
| `Dot.dc.html` | M · Scarlet case, white dot LCD: a red powder-coat case, a white dot-matrix display, silver knobs, white key lights over held keys | Kept for later |
| `Gloss.dc.html` | N · Scarlet gloss lacquer: piano-gloss red with gold trim, a champagne-on-black OLED in a gold bezel, gold knobs and gold-ringed touch buttons | Kept for later |

The `.dc.html` files are Design Component pages for the claude.ai design canvas. They
need its runtime (`support.js`), so open them through the canvas link above rather than
straight in a browser.
