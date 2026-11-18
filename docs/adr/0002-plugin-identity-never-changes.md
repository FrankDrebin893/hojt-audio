# A plugin's identity never changes once it is in use

A new build replaces the old one in place, and every DAW project picks it up with its settings. For that to hold, these are frozen for each plugin once it exists: the bundle name in `bundler.toml`, `NAME`, `VST3_CLASS_ID`, `CLAP_ID`, the `#[id]` of every parameter and the keys of every `#[persist]` field. Adding parameters is fine; removing or renaming an id loses that setting in saved projects.

The vendor string is the ASCII `Hojt Audio` (`suite_common::VENDOR`), not `HØJT`: hosts show it in lists and paths, and a non-ASCII character there is a risk we do not need. The brand on the page, in the releases and in the README stays `HØJT` (`BRAND`). The Pitch Shifter's two ids predate the suite and are kept as they were.

## Consequences

The name a DAW lists a plugin under cannot be improved later. A better name for a plugin means a new plugin (new ids, and the old one stays installed), or a change only on the page and in the README.

Each plugin's `ids` test holds this against its `ids.txt`; the procedure (`UPDATE_IDS=1`) is in `plugins/README.md`, "Versioning".
