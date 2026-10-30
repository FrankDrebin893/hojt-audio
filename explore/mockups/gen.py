import os, sys
out = sys.argv[1]
P = [  # id, name, model, kind, img, paint, w, h
 ("guitar-amp","Guitar Amp","GA-3","effect","guitar-amp.png","oxblood",1590,694),
 ("drum-synth","Drum Synth","DS-2","instrument","drum-synth.png","teal",660,600),
 ("pitch-shifter","Pitch Shifter","PS-10","effect","pitch-shifter.png","orange",400,560),
 ("bass-synth","Bass Synth","BA-1","instrument","bass-synth.png","cobalt",660,600),
 ("shimmer-pad","Shimmer Pad","SP-1","instrument","shimmer-pad.png","plum",660,600),
 ("glass-bells","Glass Bells","GB-1","instrument","glass-bells.png","pine",660,600),
]
CLIPS = ["Guitar, straight in","Klar, clean","Brøl, crunch","Glød, edge of breakup","Torden, high gain","Torden, lead with delay","Preset Tåge: clean, long delay and reverb"]
LENS = ["0:21","0:21","0:21","0:21","0:21","0:24","0:28"]
def page(tag, title, body):
    return f'''<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<link rel="stylesheet" href="../hojt.css"><link rel="stylesheet" href="../dsp/style.css"><link rel="stylesheet" href="explore.css">
</head><body>
<header class="site-bar"><div class="wrap">
<a class="home" href="../"><img src="../hojt.svg" width="63" height="34" alt="HØJT, home"></a>
<nav aria-label="Products"><a class="dsp" href="../dsp/" aria-current="page">HØJT DSP</a><a class="piano" href="../piano/">Piano viz</a><a class="resources" href="../resources/">Resources</a><a class="source" href="#">Source</a></nav>
</div></header>
{body}
<div class="mock-tag">{tag}</div>
</body></html>'''
SAY = '<p class="say">Free plugins for guitar, drums, bass and keys</p>'
INTRO = '''<p>HØJT is Danish for loud, and for high. HØJT DSP is six plugins, each drawn as a
 stompbox: an amp with a board of pedals, a drum kit, a pitch shifter, a bass, an ambient
 pad and glass bells. No sign-up, no ads, no samples to install.</p>'''
FACTS = '<ul class="facts"><li>Free</li><li>VST3 + CLAP</li><li>Windows / macOS / Linux</li><li>Open source, GPL-3.0</li></ul>'
NEXT = '''<section class="plugin oxblood"><div class="paint"><div class="wrap"><h2 class="band">Guitar Amp</h2></div></div>
<div class="wrap"><p class="caption"><b>GA-3</b> / Effect / mono or stereo in, stereo out</p></div></section>'''
files = {}
# Hero A
items = "".join(f'<li class="{p[5]}"><a href="#{p[0]}"><img src="../img/{p[4]}" width="{p[6]}" height="{p[7]}" alt="{p[1]}"><span class="tag">{p[1]} <small>{p[2]}</small></span></a></li>' for p in P)
files["hero-a.html"] = page("Hero A: Bench lineup", "Hero A", f'''<div class="top slim"><div class="wrap">
<h1 class="band">HØJT DSP</h1><div class="ahead">{SAY}{INTRO}</div>
<ul class="lineup">{items}</ul>{FACTS}</div></div>{NEXT}''')
# Hero B
board = "".join(f'<li class="{p[5]}"><a href="#{p[0]}"><img src="../img/{p[4]}" width="{p[6]}" height="{p[7]}" alt="{p[1]}"><span>{p[1]}</span><small>{p[2]} / {p[3]}</small></a></li>' for p in P[1:])
files["hero-b.html"] = page("Hero B: Rig and board", "Hero B", f'''<div class="top slim"><div class="wrap">
<h1 class="band">HØJT DSP</h1>
<div class="rig-head">{SAY}{INTRO}</div>
<div class="rig"><a class="amp" href="#guitar-amp"><img src="../img/guitar-amp.png" width="1590" height="694" alt="Guitar Amp"></a>
<p class="amp-tag">Guitar Amp <small>GA-3 / effect / four amps, ten pedals</small></p>
<ul class="pedalboard">{board}</ul></div>{FACTS}</div></div>{NEXT}''')
# Hero C
sel = "".join(f'<li class="{p[5]}"><a href="#{p[0]}"{" aria-current=\"true\"" if p[0]=="drum-synth" else ""}><span class="led"></span>{p[1]}<small>{p[2]}</small></a></li>' for p in P)
files["hero-c.html"] = page("Hero C: Switcher", "Hero C", f'''<div class="top slim"><div class="wrap">
<h1 class="band">HØJT DSP</h1>
<div class="switch teal"><div>{SAY}{INTRO}<ul class="selector">{sel}</ul></div>
<a class="stage" href="#drum-synth"><span class="go">DS-2 / instrument / to its section</span><img src="../img/drum-synth.png" width="660" height="600" alt="Drum Synth"></a></div>
{FACTS}</div></div>{NEXT}''')

# Players: in the Guitar Amp section context, What it is beside Listen
WHAT = '''<h3>What it is</h3><p>A whole guitar rig in one window: four amps, each with its own cabinet, and ten
 pedals around them. Plug a guitar straight into the audio interface and play.</p>
<ul class="points"><li><b>Klar</b> is clean and glassy, <b>Brøl</b> a mid-forward crunch, <b>Torden</b> a tight modern high gain, <b>Glød</b> warm and chiming at the edge of breakup</li>
<li>Gate, compressor, overdrive, phaser, chorus, flanger, tremolo, cabinet, delay and reverb</li><li>0.1 to 0.3 ms of latency, so it can be played live</li></ul>'''
NOTE = '<p class="note">The guitar in these clips is made by a program, not played, so the amp is all you hear change.</p>'
def sect(listen, tag, extra=""):
    return page(tag, tag.split(":")[0], f'''<section class="plugin oxblood ctx"><div class="paint"><div class="wrap"><h2 class="band">Guitar Amp</h2></div></div>
<div class="wrap"><p class="caption"><b>GA-3</b> / Effect / mono or stereo in, stereo out</p>
<div class="cols"><div>{WHAT}</div><div><h3>Listen</h3>{listen}{NOTE}</div></div>{extra}</div></section>''')
rows = "".join(f'''<li class="{"playing" if i==2 else ""}"><button class="fs{" on" if i==2 else ""}" aria-label="Play {c}"></button><span class="led{" on" if i==2 else ""}"></span><span class="name">{c}</span><span class="time">{"0:08 / " if i==2 else ""}{LENS[i]}</span><span class="bar"><i style="width:38%"></i></span></li>''' for i,c in enumerate(CLIPS))
files["player-a.html"] = sect(f'<ul class="tapes">{rows}</ul>', "Player A: Tape rows")
tabs = "".join(f'<li><button class="{"sel" if i==2 else ""}"><span class="led{" on" if i==2 else ""}"></span>{c}</button></li>' for i,c in enumerate(CLIPS))
files["player-b.html"] = sect(f'''<div class="deck"><div class="face"><button class="fs on" aria-label="Pause"></button>
<div class="lcd"><div class="row"><span class="now">Brøl, crunch</span><span class="time">0:08 / 0:21</span></div><div class="scrub"><i style="width:38%"></i></div></div></div>
<ul class="clips">{tabs}</ul></div>''', "Player B: Audition deck")
cells = "".join(f'''<li class="{"playing" if i==2 else ""}"><button><span class="led{" on" if i==2 else ""}"></span><span class="name">{c}</span><span class="len">{"0:08 / " if i==2 else ""}{LENS[i]}</span>{'<span class="fill" style="width:38%"></span>' if i==2 else ""}</button></li>''' for i,c in enumerate(CLIPS))
files["player-c.html"] = sect(f'<ul class="rack">{cells}</ul>', "Player C: Tape rack")

# Downloads: Guitar Amp section (What it is + Download) and the Install section with all six
Z = "https://github.com/FrankDrebin893/hojt-audio/releases/latest/download/"
ZN = {"guitar-amp":"HojtGuitarAmp","drum-synth":"HojtDrumSynth","pitch-shifter":"HojtPitchShifter","bass-synth":"HojtBassSynth","shimmer-pad":"HojtShimmerPad","glass-bells":"HojtGlassBells"}
def keys(pid, cls="key-s"):
    return "".join(f'<a class="{cls}" href="{Z}{ZN[pid]}-{os_}.zip">{lab}</a>' for os_,lab in [("windows","Windows"),("macos","macOS"),("linux","Linux")])
INSTALL_STEPS = '''<ol><li>Download the zip for your system and unpack it.</li><li>Copy <b>one</b> of the two plugin files to its folder: <code>.vst3</code> for most DAWs, or <code>.clap</code> where the DAW supports it.</li><li>Let the DAW scan for plugins. They are listed under <b>Hojt Audio</b>.</li></ol>'''
DISC = '<p class="note-once">Windows: VST3 + CLAP, 64-bit. macOS: Intel + Apple Silicon. Linux: x86-64. The macOS and Linux builds have not been tried in a DAW yet.</p>'
def dlpage(tag, insection, install, caption=None):
    cap = caption or '<p class="caption"><b>GA-3</b> / Effect / mono or stereo in, stereo out</p>'
    return page(tag, tag.split(":")[0], f'''<main><section class="plugin oxblood ctx"><div class="paint"><div class="wrap"><h2 class="band">Guitar Amp</h2></div></div>
<div class="wrap">{cap}<div class="cols"><div>{WHAT}{insection}</div><div><h3>Listen</h3><ul class="demos"><li><span>Guitar, straight in</span></li><li><span>Klar, clean</span></li><li><span>Brøl, crunch</span></li></ul></div></div></div></section>
<div class="wrap"><p class="gap-note">the five other plugins, each the same</p></div>
<section class="install" id="install" style="margin-top:0"><div class="wrap"><h2>Download and install</h2>{install}</div></section></main>''')
shelf = "".join(f'<div class="unit {p[5]}"><b>{p[1]}</b><small>{p[2]} / {p[3]}</small><div class="keys">{keys(p[0])}</div></div>' for p in P)
files["download-a.html"] = dlpage("Download A: Strip and shelf",
  f'<h3>Download</h3><div class="dl-strip">{keys("guitar-amp")}<a class="more" href="#install">How to install</a></div>',
  f'<div class="shelf">{shelf}</div>{DISC}{INSTALL_STEPS}')
mrows = "".join(f'<tr class="{p[5]}"><td><span class="pname">{p[1]}<small>{p[2]} / {p[3]}</small></span></td>' + "".join(f'<td><a class="key-s" href="{Z}{ZN[p[0]]}-{o}.zip">{l}</a></td>' for o,l in [("windows","Windows"),("macos","macOS"),("linux","Linux")]) + '</tr>' for p in P)
files["download-b.html"] = dlpage("Download B: One matrix",
  '<h3>Download</h3><p><a class="jumpdl" href="#install">Download the Guitar Amp ↓</a></p>',
  f'<table class="matrix"><thead><tr><th>Plugin</th><th>Windows<br><small>VST3 + CLAP, 64-bit</small></th><th>macOS<br><small>Intel + Apple Silicon</small></th><th>Linux<br><small>x86-64</small></th></tr></thead><tbody>{mrows}</tbody></table>{DISC}{INSTALL_STEPS}')
oslinks = "".join(f'<a href="{Z}HojtGuitarAmp-{o}.zip">{l}</a>' for o,l in [("windows","Windows"),("macos","macOS"),("linux","Linux")])
files["download-c.html"] = dlpage("Download C: On the caption",
  '', f'{DISC}{INSTALL_STEPS}',
  caption=f'<div class="capbar"><p class="caption"><b>GA-3</b> / Effect / mono or stereo in, stereo out</p><p class="oslinks"><span class="lbl">Download ↓</span>{oslinks}<a href="#install">Install</a></p></div>')
for n,s in files.items():
    open(os.path.join(out,n),"w").write(s)
print(len(files))
