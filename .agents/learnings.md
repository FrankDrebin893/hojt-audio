# Learnings

What Rasmus said about a result, kept so the next run does not make the same mistake. Written by
`/feedback` (`.agents/skills/feedback/SKILL.md`); read by every skill before it starts, the
section of its own process and **Everywhere**.

Each entry: the date, where it came from (issue or pull request), what was wrong, and the rule
to follow from now on. A rule here is as binding as the skill's own text; where the two
disagree, the newer one wins until `/feedback` folds the learning into the skill. When a skill
is changed to carry a learning, its entry stays here with `In the skill:` and the place.

## Everywhere

- **2026-11-12, Builder report (#247, #280).** The nightly report listed bare numbers
  ("#247 waits", "#280 skipped"), and Rasmus could not tell whether each was an issue or a pull
  request, what it was about, or what he was meant to do. **Rule:** in every report, comment
  and chat message, never cite a bare number. Each item says whether it is an issue or a pull
  request, gives its title in plain words and a link, and says in one line whether it needs
  anything from Rasmus and exactly what (for example "read the three plate renders and pick
  one", or "nothing, it is merged"). An item waiting on him names the question, not only the
  label. Applies to every routine (Scout, Specwriter, Builder, Stall check, Release Manager,
  Release Analyst, the dashboard digest) and to this repository's skills.

## Design (`/design`)

- **2026-10-31, #190 (explore, editors and piano viz).** None of the three directions was worth
  building. The mock-ups had many overlapping elements (a label run under the toggle beside it,
  the pedal art behind the footswitches), and the directions differed too little: B was A plus
  a little, C was the smallest change, and all three only re-printed the same layout.
  **Rule:** every direction is a change big enough that Rasmus would build it on its own: a
  different layout, a different structure or a different idea of the product, never the same
  screen re-skinned, and never one direction contained in another. At least one is bold enough
  to feel risky. Before posting, look at each render for overlapping, clipped or colliding
  elements and fix them; a mock-up with a collision is not shown. In the skill: `/design`,
  explore steps 2 and 3.

## Maps (`/wayfinder`)

## Scout (`/scout`)

## Specs (`/groom`, `/spec`, `/specwriter`, `/issue`)

## Builds (`/next`)

- **2026-11-03, stall check, #182, #175, #190.** #182 and #175 were done (picks recorded, every
  sub-issue closed) but stayed open with `state:building` through two Builder runs, because the
  sweep only handled closed issues and parents. #190 (explore) got the engineer's pick on
  2026-11-02, but nobody recorded it or closed the issue, so #191 (`Waits for #190`) stayed blocked.
  **Rule:** the start-of-run sweep also records a pick and closes a finished explore issue, and
  closes an open `state:building` issue whose dependents are all closed. In the skill: `/next`,
  orchestrator step 1, "Sweep, finished explore".
- **2026-11-12, #280.** A builder put `needs:engineer` on an issue for "a pick between plate
  designs" before any designs existed, so the Builder skipped it and the explore that makes the
  designs never ran. **Rule:** `needs:engineer` goes on only once the question can be answered:
  the explore renders are posted, or the question is written out in the comment. A step the
  agent can do itself (the explore) is done first, then asked.

## Page and releases (`/page`, `/release`, `/piano-release`, `/piano-readme`)

## Release cycle (`/release-prep`, `/release-review`)
