#!/usr/bin/env python3
"""Mock-ups of the three layout directions for #261, at the amp's real window size 3204x650."""
import sys, os, html
W, H = 3204, 650
FONTS = "/home/user/hojt-audio/crates/suite_common/assets/fonts"
OUT = os.path.dirname(os.path.abspath(__file__))

CSS = """
@font-face{font-family:Bebas;src:url('file://%(f)s/BebasNeue-Regular.ttf')}
@font-face{font-family:Bowlby;src:url('file://%(f)s/BowlbyOneSC-Regular.ttf')}
@font-face{font-family:Plex;src:url('file://%(f)s/IBMPlexMono-Medium.ttf')}
*{box-sizing:border-box;margin:0;padding:0}
body{width:%(w)dpx;height:%(h)dpx;background:#131211;position:relative;overflow:hidden;font-family:Bebas;color:#eadfc8}
.a{position:absolute}
.cap{font-family:Bebas;letter-spacing:1.5px;color:#eadfc8;font-size:15px;line-height:15px;text-align:center;white-space:nowrap}
.tape{font-family:Plex;font-size:12px;line-height:16px;height:16px;background:#14110f;color:#eadfc8;text-align:center;border-radius:2px;white-space:nowrap}
.knobc{border-radius:50%%;background:radial-gradient(circle at 38%% 30%%,#6a6159 0,#2b2723 45%%,#15130f 100%%);box-shadow:0 3px 5px rgba(0,0,0,.6),inset 0 0 0 2px #0d0c0a}
.knobc:after{content:'';position:absolute;left:50%%;top:7%%;width:12%%;height:30%%;margin-left:-6%%;background:#eadfc8;border-radius:2px;transform:rotate(var(--rot,0deg));transform-origin:50%% 170%%}
.led{border-radius:50%%;background:#3a1512;box-shadow:inset 0 0 0 2px #0a0a0a}
.led.on{background:radial-gradient(circle,#ff9a80,#e8281a 60%%);box-shadow:0 0 6px #e8281a,inset 0 0 0 2px #2a0b08}
.foot{border-radius:50%%;background:radial-gradient(circle at 40%% 30%%,#e4e4e4,#9a9a9a 55%%,#555);box-shadow:0 3px 5px rgba(0,0,0,.6),inset 0 0 0 3px #2c2c2c}
.screw{width:9px;height:9px;border-radius:50%%;background:radial-gradient(circle,#bbb,#555);}
.chrome{background:linear-gradient(#e9e9e9,#9c9c9c 55%%,#6c6c6c);border-radius:4px;box-shadow:0 1px 2px #000}
.plate{background:linear-gradient(#242220,#161412);border-radius:6px;border:2px solid #8a8a88;box-shadow:0 8px 24px rgba(0,0,0,.7),inset 0 0 0 1px #000}
.led-d{background:#0c0c0e;border:1px solid #3a3a40;border-radius:3px;font-family:Plex;color:#e6f0ff;font-size:17px;display:flex;align-items:center;padding-left:8px;white-space:nowrap}
.row{background:#14110f;border-radius:2px;font-family:Plex;font-size:13px;color:#eadfc8;display:flex;align-items:center;padding-left:10px;white-space:nowrap}
.plusw{border-radius:50%%;background:radial-gradient(circle at 40%% 30%%,#fff6dc,#e8c46a 60%%,#a67c1c);color:#2a2622;font-family:Bebas;display:flex;align-items:center;justify-content:center;box-shadow:0 0 0 3px rgba(234,223,200,.25),0 3px 8px #000;font-size:40px;line-height:36px;padding-bottom:3px}
.ins{border-left:3px dashed rgba(232,196,106,.85)}
.tag{font-family:Plex;font-size:12px;color:#c9bfa8;letter-spacing:1px;white-space:nowrap}
""" % dict(f=FONTS, w=W, h=H)

PEDALS = {
 'gate':   dict(name='GATE',   c='#5f6266', k=['INPUT','THRESH','RELEASE'], v=['0.0 dB','-60 dB','100 ms']),
 'pitch':  dict(name='PITCH',  c='#d6641b', k=['SEMIS','BLEND','TONE'], v=['0','100 %','5.0']),
 'fuzz':   dict(name='FUZZ',   c='#a8261c', k=['FUZZ','TONE'], v=['5.0','5.0']),
 'comp':   dict(name='COMP',   c='#6c8c1e', k=['LEVEL','SUSTAIN'], v=['5.0','5.0']),
 'drive':  dict(name='DRIVE',  c='#cf7d14', k=['DRIVE','TONE','LEVEL'], v=['3.0','5.0','5.0']),
 'phaser': dict(name='PHASER', c='#5b2f98', k=['RATE','DEPTH'], v=['0.50 Hz','50 %']),
 'chorus': dict(name='CHORUS', c='#1d4aa8', k=['RATE','DEPTH'], v=['0.80 Hz','50 %']),
 'flanger':dict(name='FLANGER',c='#1f8a4c', k=['RATE','DEPTH'], v=['0.30 Hz','50 %']),
 'trem':   dict(name='TREMOLO',c='#a02f78', k=['RATE','DEPTH'], v=['5.00 Hz','50 %']),
 'eq':     dict(name='EQ',     c='#7a4a22', k=['LOW','MID','HIGH','FREQ'], v=['5.0','5.0','5.0','5.0']),
 'delay':  dict(name='DELAY',  c='#16808a', k=['TIME','REPEAT','MIX'], v=['350 ms','35 %','25 %']),
 'reverb': dict(name='REVERB', c='#1f2e68', k=['DECAY','MIX'], v=['1.5 s','20 %']),
}

def shade(c, f):
    c = c.lstrip('#'); r, g, b = [int(c[i:i+2], 16) for i in (0, 2, 4)]
    return '#%02x%02x%02x' % tuple(max(0, min(255, int(v * f))) for v in (r, g, b))

def esc(s): return html.escape(s)

def div(x, y, w, h, cls='', style='', inner='', c=False, over=False):
    cc = ' c' if c else ''
    ov = ' data-over="1"' if over else ''
    return f'<div{ov} class="a {cls}{cc}" style="left:{x}px;top:{y}px;width:{w}px;height:{h}px;{style}">{inner}</div>'

def knob(x, y, r, label, val, rot=-40):
    """Group box: label above, knob, value tape below. Box = (2r+24) x (2r+40)."""
    gw = 2 * r + 24
    inner = (f'<div class="a cap" style="left:0;top:0;width:{gw}px">{esc(label)}</div>'
             f'<div class="a knobc" style="left:12px;top:19px;width:{2*r}px;height:{2*r}px;--rot:{rot}deg"></div>'
             f'<div class="a tape" style="left:4px;top:{2*r+22}px;width:{gw-8}px">{esc(val)}</div>')
    return div(x, y, gw, 2 * r + 40, inner=inner, c=True)

def pedal(x, y, w, h, key, r=30, cls_extra='', selected=False):
    p = PEDALS[key]; c = p['c']
    n = len(p['k'])
    gw = 2 * r + 24; gh = 2 * r + 40
    bg = f'linear-gradient(160deg,{shade(c,1.18)},{c} 45%,{shade(c,.78)})'
    s = []
    s.append(f'<div class="a" style="left:0;top:0;width:{w}px;height:{h}px;border-radius:10px;background:{bg};box-shadow:0 8px 16px rgba(0,0,0,.65),inset 0 0 0 2px {shade(c,1.35)},inset 0 -4px 0 {shade(c,.6)}"></div>')
    # art: two large faint rings
    s.append(f'<div class="a" style="left:{w*0.15}px;top:{h*0.34}px;width:{w*0.7}px;height:{w*0.7}px;border-radius:50%;border:5px solid {shade(c,.82)};opacity:.5"></div>')
    for sx, sy in ((7, 7), (w - 16, 7), (7, h - 16), (w - 16, h - 16)):
        s.append(f'<div class="a screw" style="left:{sx}px;top:{sy}px"></div>')
    s.append(f'<div class="a" style="left:0;top:12px;width:{w}px;text-align:center;font-family:Bowlby;font-size:20px;color:#eadfc8;letter-spacing:1px">{p["name"]}</div>')
    top = 52
    if n == 2:
        pos = [((w // 2 - gw - 4), top + 28), (w // 2 + 4, top + 28)]
    elif n == 3:
        pos = [((w // 2 - gw - 4), top), (w // 2 + 4, top), (w // 2 - gw // 2, top + gh + 6)]
    else:
        pos = [((w // 2 - gw - 4), top), (w // 2 + 4, top), ((w // 2 - gw - 4), top + gh + 6), (w // 2 + 4, top + gh + 6)]
    for i, (kx, ky) in enumerate(pos):
        s.append(knob(kx, ky, r, p['k'][i], p['v'][i], rot=-60 + 35 * i))
    sw = 44
    s.append(f'<div class="a led on" style="left:{w//2-6}px;top:{h-sw-52}px;width:12px;height:12px"></div>')
    s.append(f'<div class="a foot" style="left:{w//2-sw//2}px;top:{h-sw-26}px;width:{sw}px;height:{sw}px"></div>')
    ring = 'outline:3px solid #e8c46a;outline-offset:3px;' if selected else ''
    return div(x, y, w, h, inner=''.join(s), style=ring, c=True)

def tile(x, y, w, h, key, selected=False):
    """C: a small chain tile showing a name, a lamp and the values."""
    p = PEDALS[key]; c = p['c']
    bg = f'linear-gradient(160deg,{shade(c,1.18)},{c} 45%,{shade(c,.78)})'
    s = [f'<div class="a" style="left:0;top:0;width:{w}px;height:{h}px;border-radius:9px;background:{bg};box-shadow:0 6px 12px rgba(0,0,0,.6),inset 0 0 0 2px {shade(c,1.35)}"></div>']
    s.append(f'<div class="a" style="left:0;top:10px;width:{w}px;text-align:center;font-family:Bowlby;font-size:19px;color:#eadfc8">{p["name"]}</div>')
    ks = list(zip(p['k'], p['v']))[:3]
    for i, (k, v) in enumerate(ks):
        yy = 46 + i * 34
        s.append(f'<div class="a cap" style="left:12px;top:{yy}px;width:78px;text-align:left;font-size:14px">{k}</div>')
        s.append(f'<div class="a tape" style="left:92px;top:{yy-1}px;width:{w-104}px">{esc(v)}</div>')
    s.append(f'<div class="a led on" style="left:16px;top:{h-34}px;width:12px;height:12px"></div>')
    s.append(f'<div class="a foot" style="left:{w-58}px;top:{h-46}px;width:34px;height:34px"></div>')
    ring = 'outline:4px solid #e8c46a;outline-offset:4px;' if selected else ''
    return div(x, y, w, h, inner=''.join(s), style=ring, c=True)

HEAD = {
 'torden': dict(name='TORDEN', plate='#1a1816', trim='#8a1d1d', cab='#242321', ink='#eadfc8'),
 'klar':   dict(name='KLAR',   plate='#e8dfca', trim='#b9a56a', cab='#d9cfb4', ink='#2a2622'),
}
AMP_KN = [('GAIN','5.0'),('BASS','5.0'),('MID','5.0'),('TREBLE','5.0'),('PRES','5.0'),('MASTER','5.0'),('LEVEL','0 dB')]

def lane(x, y, w, h, which, num, cabname, r=15, on=True, small=False, ky=50):
    """An amp lane: head (name, knobs, cabinet stepper) and the cabinet beside it."""
    a = HEAD[which]; ink = a['ink']
    cw = int(w * 0.30); hw = w - cw - 8
    s = []
    s.append(f'<div class="a" style="left:0;top:0;width:{hw}px;height:{h}px;border-radius:8px;background:{a["plate"]};box-shadow:0 8px 14px rgba(0,0,0,.65),inset 0 0 0 3px {a["trim"]}"></div>')
    s.append(f'<div class="a" style="left:{hw//2-80}px;top:10px;width:160px;height:30px;border-radius:4px;background:#0c0b0a;border:2px solid {a["trim"]};text-align:center;font-family:Bowlby;font-size:19px;line-height:26px;color:#eadfc8">{a["name"]}</div>')
    n = len(AMP_KN); gw = 2 * r + 24
    step = (hw - 40) / n
    for i, (k, v) in enumerate(AMP_KN):
        kx = int(20 + i * step + (step - gw) / 2)
        s.append(knob(kx, ky, r, k, v, rot=-50 + 15 * i))
    yb = ky + 2 * r + 40 + 12
    s.append(f'<div class="a cap" style="left:16px;top:{yb+5}px;color:{ink};font-size:17px">AMP {num}</div>')
    s.append(f'<div class="a tape" style="left:{hw//2-70}px;top:{yb+3}px;width:140px;background:#14110f">{esc(cabname)}</div>')
    s.append(f'<div class="a led {"on" if on else ""}" style="left:{hw-70}px;top:{yb+6}px;width:11px;height:11px"></div>')
    s.append(f'<div class="a foot" style="left:{hw-48}px;top:{yb-2}px;width:30px;height:30px"></div>')
    if h - (yb + 40) > 60:
        s.append(f'<div class="a" style="left:14px;top:{yb+44}px;width:{hw-28}px;height:{h-yb-58}px;border-radius:4px;background:repeating-linear-gradient(45deg,#6b5a2e 0 3px,#8a7440 3px 6px);box-shadow:inset 0 0 0 2px {a["trim"]}"></div>')
    # cabinet
    cc = a['cab']
    s.append(f'<div class="a" style="left:{hw+8}px;top:0;width:{cw}px;height:{h}px;border-radius:6px;background:{cc};box-shadow:0 8px 14px rgba(0,0,0,.65),inset 0 0 0 3px {a["trim"]}"></div>')
    s.append(f'<div class="a" style="left:{hw+8+14}px;top:14px;width:{cw-28}px;height:{h-28}px;border-radius:4px;background:repeating-linear-gradient(45deg,#2a2826 0 3px,#3a3835 3px 6px);opacity:.9"></div>')
    return div(x, y, w, h, inner=''.join(s), c=True)

def plus(x, y, d=44, hover=True):
    return div(x, y, d, d, 'plusw', inner='+', c=True, style=f'font-size:{int(d*0.9)}px')

def cable(pts, color='#cfcfcf'):
    d = 'M' + ' L'.join(f'{x},{y}' for x, y in pts)
    return f'<path d="{d}" fill="none" stroke="#050505" stroke-width="9" stroke-linecap="round" stroke-linejoin="round"/><path d="{d}" fill="none" stroke="{color}" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" opacity=".9"/>'

def svg(paths):
    return f'<svg class="a" style="left:0;top:0" width="{W}" height="{H}">{"".join(paths)}</svg>'

# ---- strip pieces -------------------------------------------------------------------------
def icon_metro(x, y, s=34):
    return div(x, y, s, s, inner=f'<svg width="{s}" height="{s}" viewBox="0 0 34 34"><path d="M10 29 L14 5 H20 L24 29 Z" fill="none" stroke="#eadfc8" stroke-width="2.4" stroke-linejoin="round"/><path d="M17 24 L22 9" stroke="#eadfc8" stroke-width="2.4"/><circle cx="21" cy="12" r="2.6" fill="#eadfc8"/></svg>', c=True)

def icon_tuner(x, y, s=34):
    return div(x, y, s, s, inner=f'<svg width="{s}" height="{s}" viewBox="0 0 34 34"><path d="M11 4 V15 a6 6 0 0 0 12 0 V4 M17 21 V30" fill="none" stroke="#eadfc8" stroke-width="2.4" stroke-linecap="round"/></svg>', c=True)

def icon_gear(x, y, s=34):
    return div(x, y, s, s, inner=f'<svg width="{s}" height="{s}" viewBox="0 0 34 34"><circle cx="17" cy="17" r="5" fill="none" stroke="#eadfc8" stroke-width="2.4"/><path d="M17 3v5M17 26v5M3 17h5M26 17h5M7 7l3.5 3.5M23.5 23.5L27 27M27 7l-3.5 3.5M10.5 23.5L7 27" stroke="#eadfc8" stroke-width="2.6" stroke-linecap="round"/></svg>', c=True)

def mini_switch(x, y, on=False):
    return (div(x, y, 12, 12, 'led ' + ('on' if on else ''), c=True) +
            div(x + 18, y - 4, 20, 20, 'foot', c=True))

def jack(x, y, label, arrow=''):
    return (div(x, y, 20, 20, style='border-radius:50%;background:radial-gradient(circle,#111 35%,#aaa 40%,#555);', c=True) +
            div(x - 10, y + 24, 40, 14, 'cap', inner=label, style='font-size:12px;line-height:12px'))

def preset_btn(x, y, w, h, open_=False):
    return div(x, y, w, h, style='background:#0f0d0c;border:2px solid #8a8a88;border-radius:5px;display:flex;align-items:center;justify-content:space-between;padding:0 12px;font-family:Plex;font-size:18px;color:#eadfc8;',
               inner=f'<span>FACTORY &middot; INIT</span><span style="font-size:14px">&#9662;</span>', c=True)

def meter(x, y, w, h, n=10, lit=3, horizontal=True):
    s = ''
    if horizontal:
        lw = (w - (n - 1) * 3) / n
        for i in range(n):
            col = '#2e8b3a' if i < 7 else ('#d8a31a' if i < 9 else '#c82a1a')
            s += f'<div class="a" style="left:{i*(lw+3)}px;top:0;width:{lw}px;height:{h}px;background:{col if i<lit else "#231f1c"};border-radius:2px"></div>'
    else:
        lh = (h - (n - 1) * 3) / n
        for i in range(n):
            j = n - 1 - i
            col = '#2e8b3a' if j < 7 else ('#d8a31a' if j < 9 else '#c82a1a')
            s += f'<div class="a" style="top:{i*(lh+3)}px;left:0;height:{lh}px;width:{w}px;background:{col if j<lit else "#231f1c"};border-radius:2px"></div>'
    return div(x, y, w, h, inner=s, c=True)

def badge(x, y, s=44):
    return div(x, y, s * 2, s, style='background:#0c0b0a;border-radius:6px;border:1px solid #444;font-family:Bowlby;color:#eadfc8;font-size:22px;display:flex;align-items:center;justify-content:center', inner='H&Oslash;JT', c=True)

def metro_plate(x, y):
    """The metronome's settings as a hardware plate that dropped from its icon (640 x 132)."""
    w, h = 780, 132
    s = []
    s.append(f'<div class="a cap" style="left:14px;top:8px;font-size:17px;text-align:left">METRONOME</div>')
    s.append(div(14, 36, 112, 40, 'led-d', inner='120', style='font-size:26px', c=True) + div(14, 80, 112, 16, 'cap', inner='TEMPO', style='font-size:12px'))
    s.append(div(138, 38, 62, 38, 'chrome', c=True, inner='<div style="font-family:Bebas;font-size:18px;color:#222;text-align:center;line-height:38px">TAP</div>'))
    for i, (cap, val) in enumerate((('SOUND', 'WOODBLOCK'), ('METER', '4/4'), ('CLICK OUT', 'MIXED'))):
        xx = 218 + i * 150
        s.append(div(xx, 38, 138, 38, 'led-d', inner=val, style='font-size:14px;padding-left:10px', c=True))
        s.append(div(xx, 80, 138, 16, 'cap', inner=cap, style='font-size:12px'))
    s.append(knob(14 + 0, 0, 0, '', '') if False else '')
    s.append(div(680, 30, 76, 90, inner=
        '<div class="a cap" style="left:0;top:0;width:76px;font-size:12px">VOLUME</div>'
        '<div class="a knobc" style="left:18px;top:20px;width:40px;height:40px;--rot:-20deg"></div>'
        '<div class="a tape" style="left:4px;top:66px;width:68px">-12.0 dB</div>', c=True))
    s.append(div(14, 104, 160, 20, 'tag', inner='SYNC TO HOST', style='font-size:12px'))
    s.append(div(150, 104, 12, 12, 'led', c=True))
    return div(x, y, w, h, 'plate', inner=''.join(s), c=True, over=True)

def bank_sheet(x, y, w, h, chain_counts=None, grid_cols=6):
    """The effects bank: a sheet that rises from below."""
    keys = ['gate','pitch','fuzz','comp','drive','phaser','chorus','flanger','trem','eq','delay','reverb']
    inner = [f'<div class="a" style="left:0;top:0;width:{w}px;height:12px;border-radius:14px 14px 0 0;background:linear-gradient(#cfcfcf,#7a7a7a)"></div>']
    inner.append(div(w // 2 - 28, 3, 56, 6, style='background:#444;border-radius:3px'))
    inner.append(f'<div class="a cap" style="left:24px;top:24px;font-size:22px;text-align:left">EFFECTS BANK</div>')
    inner.append(f'<div class="a tag" style="left:200px;top:29px">DRAG A PEDAL INTO THE CHAIN &middot; 9 OF 16 SLOTS IN USE</div>')
    inner.append(div(w - 150, 22, 120, 30, 'chrome', c=True, inner='<div style="font-family:Bebas;font-size:18px;color:#222;text-align:center;line-height:30px">CLOSE</div>'))
    cols = grid_cols; gap = 16
    tw = (w - 48 - gap * (cols - 1)) // cols; th = (h - 86 - gap) // 2
    for i, k in enumerate(keys):
        p = PEDALS[k]; c = p['c']
        col, row = i % cols, i // cols
        tx = 24 + col * (tw + gap); ty = 66 + row * (th + gap)
        used = (chain_counts or {}).get(k, 0)
        bg = f'linear-gradient(160deg,{shade(c,1.18)},{c} 45%,{shade(c,.78)})'
        t = (f'<div class="a" style="left:0;top:0;width:{tw}px;height:{th}px;border-radius:8px;background:{bg};box-shadow:inset 0 0 0 2px {shade(c,1.35)}"></div>'
             f'<div class="a" style="left:0;top:{th*0.2}px;width:{tw}px;text-align:center;font-family:Bowlby;font-size:20px;color:#eadfc8">{p["name"]}</div>'
             f'<div class="a tag" style="left:0;top:{th*0.2+36}px;width:{tw}px;text-align:center;color:#eadfc8">{" &middot; ".join(p["k"][:3])}</div>'
             f'<div class="a tag" style="left:12px;top:{th-24}px;color:#eadfc8">{"IN CHAIN &times;"+str(used) if used else "NOT IN CHAIN"}</div>')
        inner.append(div(tx, ty, tw, th, inner=t, c=True))
    return div(x, y, w, h, 'plate', style='border-radius:16px 16px 0 0;border-bottom:none;background:linear-gradient(#262321,#12100f);', inner=''.join(inner), c=True, over=True)

def ghost(x, y, w, h, key):
    p = PEDALS[key]; c = p['c']
    return div(x, y, w, h, style=f'border-radius:9px;background:{c};opacity:.85;box-shadow:0 14px 30px rgba(0,0,0,.8),inset 0 0 0 3px #e8c46a;transform:rotate(-4deg)',
               inner=f'<div class="a" style="left:0;top:{h/2-14}px;width:{w}px;text-align:center;font-family:Bowlby;font-size:20px;color:#eadfc8">{p["name"]}</div>', c=True, over=True)

def shell(body, extra=''):
    return f'<!doctype html><html><head><meta charset="utf-8"><style>{CSS}</style></head><body>{body}{extra}</body></html>'

# ---- strip: horizontal ----------------------------------------------------------------------
def hstrip(x, y, w, h, open_metro=False):
    s = []
    s.append(f'<div class="a" style="left:0;top:0;width:{w}px;height:{h}px;border-radius:10px;background:linear-gradient(#a02024,#7a1519 60%,#5c0f12);box-shadow:0 6px 12px rgba(0,0,0,.6),inset 0 0 0 2px #c0383b"></div>')
    cy = (h - 40) // 2
    # left: in, input gain, preset
    s.append(jack(24, 12, 'IN'))
    s.append(knob(80, 6, 14, '', '', rot=-30) if False else '')
    s.append(div(74, 10, 120, 12, inner='', c=False))
    s.append(div(74, 8, 60, 16, 'cap', inner='INPUT', style='font-size:13px;line-height:13px'))
    s.append(meter(74, 26, 80, 12, 10, 4))
    s.append(div(164, 8, 40, 48, inner='<div class="a knobc" style="left:2px;top:6px;width:36px;height:36px;--rot:-20deg"></div>', c=True))
    s.append(preset_btn(236, 10, 380, 44))
    # centre icons
    cx0 = 1250
    s.append(icon_metro(cx0, 14)); s.append(mini_switch(cx0 + 46, 24, False))
    s.append(icon_tuner(cx0 + 150, 14)); s.append(mini_switch(cx0 + 196, 24, False))
    s.append(icon_gear(cx0 + 300, 14))
    # right: output
    ox = w - 620
    s.append(div(ox, 8, 56, 16, 'cap', inner='OUTPUT', style='font-size:13px;line-height:13px'))
    s.append(div(ox + 60, 14, 44, 44, inner='<div class="a knobc" style="left:2px;top:6px;width:36px;height:36px;--rot:30deg"></div>', c=True))
    s.append(meter(ox + 130, 26, 120, 12, 10, 5))
    s.append(div(ox + 130, 8, 70, 16, 'cap', inner='LEVEL', style='font-size:13px;line-height:13px'))
    s.append(div(ox + 270, 12, 12, 12, 'led', c=True))
    s.append(div(ox + 258, 30, 40, 14, 'cap', inner='CLIP', style='font-size:12px;line-height:12px'))
    s.append(badge(w - 250, 10, 44))
    s.append(jack(w - 120, 12, 'OUT'))
    return div(x, y, w, h, inner=''.join(s), c=True)

def save(name, body):
    path = os.path.join(OUT, name + '.html')
    open(path, 'w').write(shell(body))
    return path
