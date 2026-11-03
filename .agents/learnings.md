# Learnings

What Rasmus said about a result, kept so the next run does not make the same mistake. Written by
`/feedback` (`.agents/skills/feedback/SKILL.md`); read by every skill before it starts, the
section of its own process and **Everywhere**.

Each entry: the date, where it came from (issue or pull request), what was wrong, and the rule
to follow from now on. A rule here is as binding as the skill's own text; where the two
disagree, the newer one wins until `/feedback` folds the learning into the skill. When a skill
is changed to carry a learning, its entry stays here with `In the skill:` and the place.

## Everywhere

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

## Page and releases (`/page`, `/release`, `/piano-release`, `/piano-readme`)

## Release cycle (`/release-prep`, `/release-review`)
