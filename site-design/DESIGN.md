---
name: HØJT site
description: The public page of HØJT (hojte.net/hojt-audio/), a dark workbench where the products are laid out under their own paint, printed like the plugins' pedals.
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

# Design System: HØJT site

The site's source of truth for its look. `site/hojt.css` holds the tokens as CSS custom
properties; this file holds why they are what they are, the rules a page is checked against and
the references. It sits outside `site/` so it is not published. How a change to the look is made
is `/design` (`.agents/skills/design/SKILL.md`).

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
What keeps HØJT clear of it is the six paints, the halftone renders and the printed labels. When
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
  window from a phone (390px wide) to a desktop (1440px) and stops there; what is not on it yet is listed under the tables. A page picks a
  step; it does not set a size of its own. A size that is not on the scale
  is a reason to add a step here, not to write a number in a page's stylesheet.

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
  - The piano viz page (`site/piano/styles.css`, #185). It already takes the shared pieces
    from `hojt.css` (body text, the top bar, buttons, label, bottom), so on a phone those are a
    step smaller there too; its own sizes stay fixed until #185.
  - The Resources pages (`site/resources/resources.css` and the two `atlas.css`): their own
    sizes are fixed; the shared pieces follow the scale as on the piano viz page.
  - Anywhere else: a size written as a step's `var()` is on the scale; a size still written in
    px is not yet, and moves onto it with #183, #185 and #186.

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

- **Label**: the product's name in Bowlby One SC on a tilted cream label over its paint band.
- **Chips**: mono, small, for facts only.
- **Download buttons**: cream keys with the platform and a mono line under it.
- **Listen**: one clip per row with its name in mono.

### Named Rules

- **No Native Chrome.** Nothing on the page looks like the browser's default: not the grey
  `<audio>` player, not a stock `<select>`, not the default focus or scrollbar. A control is
  drawn in the plugins' own vocabulary (label tape, footswitch, LED, knob).

## Voice

From `/page`: what is heard and seen, short plain sentences, no internals, no real makers' names
or trademarks.

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

## Open work (not rules)

From the audit of 2026-10-28, each one a `/design` change of its own:

- The DSP page's first screen has a headline, chips and the colour board, but no pedal.
- The Listen sections use the browser's grey `<audio>` players, up to seven in a row.
- The Download block is repeated six times on the DSP page (about 11,000px tall on desktop).
- The piano viz page breaks the voice and has its own 669-line stylesheet; it should come onto
  the tokens of `hojt.css`.
- The home page shows the logo twice in its first screen.
