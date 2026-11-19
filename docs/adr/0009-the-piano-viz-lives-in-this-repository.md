# The piano viz lives in this repository, with its own stack and release

The piano viz (C# / .NET 10 / WPF, `piano/`) shares the repository, the backlog, `main`'s protection and the site with the Rust plugins, but keeps its own licence (MIT), its own build and CI (`piano.yml`) and its own release tags (`piano-vX.Y.Z`). Its releases are never marked latest, because the plugins' page links their zips through `releases/latest/download/`. It came from `FrankDrebin893/hojt-audio-piano-viz`, which is a leftover to archive, not delete; that repository still serves the old page address.

## Considered options

- Two repositories: two backlogs and two pages for one hobbyist, for products that a player sees as one brand.

## Consequences

Bringing the piano viz's look (`piano/DESIGN.md`) together with the plugins' (`ui/STYLE.md`) is planned work.
