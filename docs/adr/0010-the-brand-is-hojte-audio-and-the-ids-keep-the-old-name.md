---
status: accepted
---
# The brand is Højte Audio, and the plugin ids keep the old name

The project was called HØJT (a cut-off form of the author's surname, Danish for "loud" and "high"). That confused people and tied the project to one person's language, so it is now **Højte Audio**: the suite is Højte DSP, the piano viz is Højte Piano, and the Ø stays in the logo and the official name. On the page the repo is `hojte-audio`, and the site stays under the author's own domain at `hojte.net/hojte-audio/`.

Only what a person reads changes. The frozen ids of ADR 0002 do not: the vendor string `Hojt Audio`, the bundle names (`HojtGuitarAmp` and the rest), the CLAP and VST3 ids, the installer's `AppId` and install folder. Renaming them would make every installed plugin a new plugin and lose saved DAW projects. So a DAW's vendor column and the `.vst3` file names say the old name, and the install page says so in a line.

## Consequences

Old page links (`hojte.net/hojt-audio/...`) keep working through small redirect stubs in the personal-site repo, because GitHub redirects a renamed repo's git and API addresses but not its Pages address. History (tags, releases, old issues, old changelog entries) is not rewritten; the changelog says "formerly HØJT" once.

If the author later learns that few people have the plugins installed, the ids can be revisited as a deliberate, separate break (ADR 0002's "new plugin" route).
