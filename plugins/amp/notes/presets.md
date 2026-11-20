# Presets, banks, dial memory and amp switching

Moved unchanged from `plugins/amp/CLAUDE.md` (issue 370); the router there says when to read it. Read it before editing the files it names.

- Each amp remembers its own six dials: the nameplate on the head stores them under the amp that
  is left and sets them to what the amp that is picked had (defaults if never left). The dials
  are still six shared parameters. A host that changes Amp itself leaves the dials alone; a
  preset's dials become its amp's memory
- An amp that is switched to starts at the dials' target, at the silent sample of the switch
- A preset load (factory or the player's) and an amp switch are told to the host as one edit
  (`presets::set_together`): every gesture begun, then every value set, then every gesture
  ended. REAPER makes that one undo point, and one undo puts the whole sound back, the preset
  display's name and the pedal order with it (checked by hand in REAPER, #16). A host that
  keeps one undo step per gesture still makes one per parameter

- A preset holds everything except Bypass, Input and Output, so every preset must fit at
  Output 0 dB (a test checks peak and level of each). It holds the chain too (#271, decision
  #260): loading a preset replaces the whole chain (blocks, order, amp lanes), saving writes it.
  A factory preset is the twelve pedals on their own slots in its order (`Preset::chain`, which
  plays as `ChainOrder` did: the preset test holds the layout and the settings to what they were), so
  every factory preset and every file from before loads as it did. What a load takes out is kept
  by effect kind in the chain's `Memory` (`StoredChain::replace`: a kind the new chain lacks, the
  first block of it in the line, read before any value is set; a kind that stays has its values
  replaced, which is not taking it out), and per amp by `dial_memory`, so the player can come
  back to it by putting the effect in again or picking the amp. New presets go at the end: a
  project remembers its preset by its number
- The player's own presets (`user_presets.rs`) are `.preset` text files in
  `Documents/Hojt Audio/Presets` (or the folder in `HOJT_PRESETS_DIR`), one `id = value` line
  per parameter by its frozen id, then `order`, `chain`, a `slot<n>` line for each block of the
  chain that is not on its own pedal's slot, and `cabinet`. They hold what a factory preset
  holds and the microphone and cabinet too. `chain` is `Chain::ids` (the project's form: `amps=2`,
  `kind@slot` for each block, `amp` between the zones); a block on its pedal's slot has its values
  in that pedal's lines, any other (a second delay, a gate) in `slot<n> = <on>:<k1>,..,<k5>`
  (`SlotValues::to_text`, knobs a kind does not use at 0), and a block with no line starts as a
  slot does (off, knobs at the middle). `order` stays as the twelve-pedal view for a build from
  before the chain. A file with no `chain` line is the twelve pedals in its `order`, with the
  amp lanes of a project from before. The preset shows as edited when the chain, a block's
  switch or a knob of a block off its pedal's slot has moved A file is read forgivingly: unknown lines are
  skipped and what it does not name gets its default. A test holds every parameter to being
  in a preset or left alone with Bypass, Input, Output and the Tuner
- Every preset is in a bank, and the Bank display on the strip says which one is showing:
  `FACTORY` (the built-in presets), `MINE` (the files loose in the presets folder, where every
  preset was kept before there were banks) or one of the folders the player made in it, in
  name order. Only the folders directly in the presets folder are banks, and what is deeper is
  left alone. The bank showing is persisted as `preset-bank`: empty for the factory bank, `/`
  for the presets folder itself and `/<folder>` for a folder in it, checked like a file name so
  it is never a path and never `..`. A project saved before there were banks has a preset of
  the player's and no bank, and opens on `MINE`
- The preset display on the strip says the bank and the preset (`FACTORY · INIT`) and is one
  click target, with no step arrows: it opens the preset plate, the banks in one column and the
  presets of the bank clicked in the other, with the foot (Save, Folder) under them. The bank
  showing changes when a preset is picked out of another bank and when a bank is made; the
  sound then stays as it stands, and with nothing loaded out of the bank the display falls back
  to the name of the preset the sound came from
- `+ NEW BANK`, at the end of the banks, makes one. Clicked, its tape takes a name; enter keeps
  it, escape drops it (the plate stays open), and a second click on the tape keeps it as it stands, so a bank can be
  made with the mouse alone. A bank nobody named is `Bank 1`, `Bank 2` and so on, the first
  that is free, renamed in the folder afterwards. A name is filtered like a file name, and one
  that already stands as a folder is simply that bank, with everything still in it
- Save, in the foot of the preset plate, writes into the bank showing and always asks for a name:
  it makes the foot's display take one, which starts as the name of the preset loaded out of
  the bank showing (empty with none loaded). Enter or a second press of Save keeps it, escape
  drops it and writes nothing, closing the plate drops it too. A name may hold spaces (#264):
  `type_into` takes a space that arrives as text, and one that arrives only as the space bar's
  key press (a host that keeps the key for play and stop), and a test holds both. The name kept as it stands writes over the
  preset loaded; any other is the file name, `<name>.preset`, with a number added when it is
  taken. A name with nothing left once the characters a file name cannot hold are taken out is
  one nobody gave, and is `<name> 2.preset` (the next free number) named after the preset the
  sound came from, as before. Nothing is written into `FACTORY`: a sound saved there goes into `MINE`, which is the
  bank showing from then on. The file name of the one loaded last is persisted as `user-preset`
  (empty after a factory preset, and after the bank changed); renaming, moving from one bank to
  another, copying and deleting is done in the folder, which the foot's Folder button opens
  (hovered it says where) — the folder of the bank showing
