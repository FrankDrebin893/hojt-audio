---
name: Højte site
description: The public page of Højte (hojte.net/hojt-audio/), a dark workbench where the products are laid out under their own paint, printed like the plugins' pedals.
colors:
  bench: "#121110"
  panel: "#1C1A18"
  tape: "#141210"
  tape-text: "#F4EFE4"
  silk: "#EADFC8"
  silk-dim: "#B9AF9B"
  ink: "#2A2622"
  line: "#3A3530"
  brand-orange: "#E2581E"
  paint-orange: "#DE561E"
  paint-teal: "#1D8282"
  paint-oxblood: "#7C1C20"
  paint-cobalt: "#1E409C"
  paint-plum: "#602C78"
  paint-pine: "#1C683C"
  paint-scarlet: "#A8131C"
  led-on: "#FF3B24"
typography:
  display:
    fontFamily: "Bebas Neue"
    use: "Headings, the top bar, buttons"
  model:
    fontFamily: "Bowlby One SC"
    use: "Product names on their cream label, as printed on the pedals"
  mono:
    fontFamily: "IBM Plex Mono"
    fontWeight: 500
    use: "Captions, model numbers, figures, the labels of the listen clips"
  body:
    fontFamily: "system-ui"
    fontSize: "var(--step-0)"
    lineHeight: 1.6
layout:
  container: "1120px"
  gutter: "max(20px, safe-area inset)"
  phone-break: "560px"
---

# Design System: Højte site

The site's source of truth for its look. `site/hojt.css` holds the tokens as CSS custom
properties; this file holds why they are what they are, the rules a page is checked against and
the references. It sits outside `site/` so it is not published. How a change to the look is made
is `/design` (`.claude/skills/design/SKILL.md`).

**Status: first draft (2026-10-28), written from the site as it is. The named rules and the
reference board are for Rasmus to confirm or correct.**

## Overview

A dark workbench at night with the gear laid out on it. The bench is near-black and quiet; the
colour comes from the products: each one has a paint, the same paint as its pedal or the piano
viz's chassis, and its name is printed on a tilted cream label the way it is on the pedal. Type is
condensed and printed (Bebas Neue, Bowlby One SC), figures and captions are in a typewriter mono.
The renders of the pedals and screenshots of the app are the best thing the site has and should
be the biggest thing on a page.

The danger: near-black with a single bright accent is one of the looks AI tools now default to.
What keeps Højte clear of it is the six paints, the halftone renders and the printed labels. When
in doubt, add more of the product, not more of the chrome.

## Colors

- **Bench** (`--bench`) is the page; **panel** (`--panel`) a raised surface on it; **line** the
  only divider colour.
- **Silk** is the text colour, **silk-dim** the second level. Never pure white.
- **Brand orange** is the Ø and nothing else that is decorative: links and the focus ring may
  use it, headings and backgrounds do not.
- **Paints** belong to products: oxblood the amp, teal the drums, orange the pitch shifter,
  cobalt the bass, plum the pad, pine the bells, scarlet the piano viz. A paint is used only next
  to its product.

### Named Rules

- **One Paint per Product.** A paint appears only in its product's section or link. Two paints
  side by side only on a board of all the products.
- **Body Text Never on Paint.** Paint is a band, a label or a chip; reading text sits on bench or
  panel (also a rule of `/page`).

## Typography

- Display (Bebas Neue) for headings and the bar, model (Bowlby One SC) only for product names on
  their label, mono for captions and figures, the system sans for reading.
- **The fluid scale.** Every size of type and space is meant to be a custom property of
  `hojt.css`, each but the smallest (`--step--3`, a fixed 12px) a `clamp()` that grows with the
  window from a phone (390px wide) to a desktop (1440px) and stops there; what is not on it
  yet is listed under the tables. A page picks a step; it does not set a size of its own. A
  size that is not on the scale is a reason to add a step here, not to write a number in a
  page's stylesheet.

  | Type | Phone to desktop | For |
  |---|---|---|
  | `--step--3` | 12px | The mono line under a button or a board tile |
  | `--step--2` | 13 to 14px | Mono: tape, chips, clip names, code, notes |
  | `--step--1` | 14 to 15px | Tables, the bottom of the page |
  | `--step-0` | 16 to 17px | Body text |
  | `--step-1` | 18 to 19px | A lead paragraph, table heads |
  | `--step-2` | 19 to 22px | The top bar, captions, the bottom nav, board tiles |
  | `--step-3` | 22 to 26px | Section headings (h3), buttons, summaries |
  | `--step-4` | 26 to 30px | The jump board's names |
  | `--step-5` | 36 to 52px | A page's section headings (h2) |
  | `--step-6` | 36 to 62px | The label (`.band`), the Install heading |
  | `--step-7` | 42 to 84px | A page's headline |

  | Space | Phone to desktop |
  |---|---|
  | `--space-2xs` | 8 to 10px |
  | `--space-xs` | 10 to 14px |
  | `--space-s` | 16 to 22px |
  | `--space-m` | 20 to 28px |
  | `--space-l` | 26 to 34px |
  | `--space-xl` | 32 to 48px |
  | `--space-2xl` | 40 to 56px |
  | `--space-3xl` | 52 to 72px: between sections |

  Each step is at least as large as the one below it at every width. Body text is `--step-0`:
  16px on a phone, 17px on a desktop.

  Not on the scale yet:
  - The home page's hero and the labels on its enclosures (#186 changes the hero).
  - The Resources pages (`site/resources/resources.css` and the two `atlas.css`): their own
    sizes are fixed; the shared pieces from `hojt.css` follow the scale.
  - Anywhere else: a size written as a step's `var()` is on the scale. A size still written in
    px is either part of a drawing (the keyboard and the app's display on the piano viz page) or
    not on the scale yet, and moves onto it with #183 and #186.

### Named Rules

- **Mono Is for Facts.** Mono only for things that are data: model numbers, figures, file names,
  the names of clips. Not for marketing chips or sentences.

## Layout

One column of 1120px at most with a 20px gutter, product sections stacked, each with its paint
band and label. A product section reads: label, caption, the render, what it is, listen,
download, controls.

### Named Rules

- **The Render Is the Hero.** A product's page shows the product in its first screen, as large
  as the layout allows. A headline and a grid of coloured tiles is not a first screen.
- **Say It Once.** A block that would be the same in every section (the download buttons, the
  install note) is made once and reused, or said once for all.

## Components

- **Top bar**: the same on every page, the home page too. It carries the Højte badge, so no page
  shows the logo again under it; the home page opens on its headline and the two products.
- **Badge**: `site/hojt.svg`, Højte in Bebas Neue, H Ø J T E in one row on the dark plate with the cream frame, the Ø a
  knob with the orange pointer. Drawn by `scripts/make_badge.py`, which also makes the favicon (the whole badge on a
  square), `favicon-32.png`, `apple-touch-icon.png` and the social image `og-hojt.png`; the plugins print the same
  badge (`ui/STYLE.md`, "The badge"). Under about 32 px the installer icon keeps only the H
- **Label**: the product's name in Bowlby One SC on a tilted cream label over its paint band.
- **Chips**: mono, small, for facts only.
- **Download matrix**: said once, at Install: a table of the six plugins by the three systems,
  each cell a cream key on a ridge of the plugin's paint. A plugin's section only links to it.
  On a phone each plugin is a card of three keys.
- **Rig and board**: the DSP page's first screen. The Guitar Amp across the page, the five
  stompboxes on a board under it, each a link to its section.
- **Audition deck**: one per plugin under Listen: a footswitch, a dark display with the clip, its
  time and a scrub bar, and tape tabs with an LED that pick the clip. Without script each clip
  keeps its own player; starting one stops any other.

### Named Rules

- **No Native Chrome.** Nothing on the page looks like the browser's default: not the grey
  `<audio>` player, not a stock `<select>`, not the default focus or scrollbar. A control is
  drawn in the plugins' own vocabulary (label tape, footswitch, LED, knob).

## Voice

Plain and dry, leaning that way, with some personality: plain, factual sentences that carry the
odd dry or warm remark where it fits, not a joke on every line. The rules, each checkable by
reading the text:

- What is heard and seen, no internals, no real makers' names or trademarks
- Nobody speaks: the page addresses "you", and no quote or character talks
- No taglines or punchlines (no "turned up"), no em-dashes, no list of three that is there only
  for its rhythm
- No vague claims ("beginner-friendly", "zero friction"): say the concrete thing instead, such
  as thirteen pedals or what a knob does
- At most one dry or warm remark in a section, and it goes no further than "Run two amps at once
  if one feels lonely"

- Right: "Four kits, each with its own room."
- Wrong: "Zero-friction first run." (marketing), "Rendered directly from the WPF visual tree at
  1.5x HiDPI scale." (internals)

## Reference board

What to borrow from each, not what to copy. Images that are our own (photos of the pedals, the
renders) may go in `site-design/refs/`; for other makers' sites keep the link and the line here,
never a copy in the repo.

- **teenage engineering** (teenage.engineering): the chrome stays black, white and grey and lets
  the product carry all the colour. "If your product is visually interesting, do not compete with
  it."
- **Valhalla DSP** (valhalladsp.com): a palette that could be screen-printed; restraint; plugin
  screenshots large and alone.
- **Chase Bliss** (chasebliss.com): art-directed demo videos, a playful voice, each pedal its own
  world.
- **FabFilter** (fabfilter.com): one screenshot per feature, rows that alternate. They have no
  sound examples; we do, so ours must look as good as their screenshots.

**For Rasmus to add:** 2 to 4 more references of your own choosing, ideally real hardware panels
and printed matter (pedal boxes, gig posters), each with one line on what to borrow.

## Do's and Don'ts

### Do

- Put the product, large, in the first screen of its page.
- Draw every control in the pedals' vocabulary.
- Check every change on screenshots at 1440 and 390 wide (`scripts/site-shots.sh`).

### Don't

- No gradient backgrounds, glassmorphism, glowing borders, or cards inside cards.
- No Inter, Roboto or Space Grotesk; no new fonts without a reason in this file.
- No made-up screenshots or sounds (`/page`: they come from `scripts/screenshots.ps1` and
  `scripts/site-audio.sh`).
- No paint as a background for reading text.

## Decided against

Product calls the engineer has made; do not propose them again without new evidence.

- **No visitor analytics** on the page (decided 2026-10-26).
- **No rendered sound examples** on the page: the renderings sounded unnatural (2026-11-14). They may come back from a real demo project, never as renderings (`/page`).

## Where the pages live

The public page (`site/`, plain HTML and CSS, no build step) is published by GitHub Pages:

- A landing page for both products at the root (https://hojte.net/hojt-audio/)
- The plugins' page in `site/dsp/` (https://hojte.net/hojt-audio/dsp/; the root sends their old `#anchors` there)
- The piano viz's page in `site/piano/` (https://hojte.net/hojt-audio/piano/)
- Resources in `site/resources/` (https://hojte.net/hojt-audio/resources/): free tools for learning to play, a
  folder each (the Guitar and Piano Chord Atlas so far) on the shelf of its `index.html`
- One look for all: `site/hojt.css`, with the plugins' colours and fonts; the favicon is the whole Højte badge
  (`site/favicon.svg`). Fonts, screenshots (`site/img/`) and sounds (`site/audio/`) stay at the root
- The old address, https://hojte.net/hojt-audio-piano-viz/, is served by the old repository
- `site-sources/` is what is played in a sound example when it is our own playing; not published

## Open work (not rules)

From the audit of 2026-10-28, each one a `/design` change of its own:

(The first screen, the player and the download block of the DSP page were done in #183 and #184, picks B of #182.)
