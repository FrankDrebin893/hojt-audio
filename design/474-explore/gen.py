#!/usr/bin/env python3
"""Mock-ups of the three layouts for #474 (one amp or two). Reuses the helpers of the #262 explore (gen262.py).
Real geometry of the stack (790 x 490, lanes 236 high, head 420, cabinet 210, plate 148 at x 642), shown in a
1100-wide cut of the 584-high window: the stack with its neighbours, so the cables and the output jack are in view."""
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen262 as g
from gen262 import div, knob, shade, esc, cable, HEAD

OUT = os.path.dirname(os.path.abspath(__file__))
CW = 1100
OX, WIN_H_FULL = 150, 584
STACK_TOP = 84
EXTRA = """
body{background:#131211}
.silk{border:1.5px solid rgba(234,223,200,.55);border-radius:5px}
.silkt{font-family:Bebas;font-size:15px;line-height:16px;letter-spacing:1.5px;color:#eadfc8;background:#16140f;text-align:center;white-space:nowrap}
.ghost{border:2px dashed rgba(234,223,200,.28);border-radius:8px}
.add{border-radius:22px;border:2px solid rgba(234,223,200,.55);background:rgba(234,223,200,.07);color:#eadfc8;font-family:Bebas;font-size:22px;letter-spacing:2px;display:flex;align-items:center;justify-content:center;gap:10px}
"""

def head_glyph(x, y, lit, w=44, h=26):
    ink = '#eadfc8' if lit else 'rgba(234,223,200,.28)'
    dots = ''.join(f'<div class="a" style="left:{6+i*9}px;top:5px;width:5px;height:5px;border-radius:50%;background:{ink}"></div>' for i in range(4))
    lamp = f'<div class="a" style="left:{w-12}px;top:5px;width:6px;height:6px;border-radius:50%;background:{"#e8281a" if lit else "#3a1512"}"></div>'
    grille = f'<div class="a" style="left:5px;top:14px;width:{w-10}px;height:7px;border-radius:2px;background:repeating-linear-gradient(90deg,{ink} 0 1px,transparent 1px 4px);opacity:.8"></div>'
    return div(x, y, w, h, style=f'border:2px solid {ink};border-radius:4px;', inner=dots + lamp + grille, c=True)

def amps_icon(x, y, w, two, h=54):
    """The mode icon in its frame: two small heads, one or two lit, 'Amps' on the frame."""
    s = [div(0, 8, w, h - 8, 'silk')]
    s.append(div(w // 2 - 28, 0, 56, 16, 'silkt', inner='AMPS', c=True))
    gx = (w - (2 * 44 + 10)) // 2
    s.append(head_glyph(gx, 20, True)); s.append(head_glyph(gx + 54, 20, two))
    return div(x, y, w, h, inner=''.join(s))

def plate(x, y, w, h, two, blend=True):
    cx = w // 2
    s = [f'<div class="a" style="left:0;top:0;width:{w}px;height:{h}px;border-radius:6px;background:linear-gradient(#1c1b1a,#0e0d0d);border:2px solid #8a8a88;box-shadow:0 8px 24px rgba(0,0,0,.7)"></div>']
    for sx, sy in ((6, 6), (w - 15, 6), (6, h - 15), (w - 15, h - 15)):
        s.append(f'<div class="a screw" style="left:{sx}px;top:{sy}px"></div>')
    s.append(amps_icon(10, 10, w - 20, two))
    if blend:
        s.append(div(10, 76, w - 20, 120, 'silk'))
        s.append(div(cx - 30, 68, 60, 16, 'silkt', inner='BLEND', c=True))
        s.append(div(cx - 12 - 30, 92, 60, 15, 'cap', inner='AMP 1', style='font-size:15px', c=True))
        s.append(div(cx - 12 - 4, 112, 8, 48, style='background:#050505;border-radius:4px;box-shadow:inset 0 0 0 1px #555', c=True))
        s.append(div(cx - 12 - 11, 130, 22, 12, 'chrome'))
        s.append(div(cx - 12 - 30, 168, 60, 15, 'cap', inner='AMP 2', style='font-size:15px', c=True))
        s.append(div(cx + 22, 128, 44, 16, 'tape', inner='50 %', c=True))
    s.append(div(10, 206, w - 20, 238, 'silk'))
    s.append(div(cx - 40, 198, 80, 16, 'silkt', inner='CABINETS', c=True))
    dx = 31 if w > 130 else 28
    s.append(knob(cx - dx - 27, 220, 15, 'MIC', '5.0', rot=-20))
    s.append(knob(cx + dx - 27, 220, 15, 'RES', '5.0', rot=20))
    s.append(div(cx - 40, 323, 80, 15, 'cap', inner='MIC TYPE', style='font-size:14px', c=True))
    s.append(div(cx - 22, 345, 44, 20, 'tape', inner='57', c=True))
    s.append(div(cx - 30 - 25, 378, 50, 15, 'cap', inner='CABS', style='font-size:14px', c=True))
    s.append(div(cx - 30 - 6, 404, 12, 12, 'led on', c=True))
    s.append(div(cx + 22 - 10, 397, 20, 20, 'foot', c=True))
    s.append(div(cx + 16 - 10, 452, 20, 20, style='border-radius:50%;background:radial-gradient(circle,#111 35%,#aaa 40%,#555);', c=True))
    s.append(div(cx + 16 - 22 - 36, 455, 36, 14, 'cap', inner='OUT', style='font-size:15px;line-height:14px', c=True))
    return div(x, y, w, h, inner=''.join(s)), (x + cx + 16, y + 462)

def grille_cloth(c='#2a2826'):
    return f'repeating-linear-gradient(45deg,{c} 0 3px,#3a3835 3px 6px)'

def head(x, y, w, h, which, num, big=False, ph_big=290):
    """One head: the panel (jack, six dials, lamp, name), the grille with the nameplate."""
    a = HEAD[which]; ink = a['ink']
    s = [f'<div class="a" style="left:0;top:0;width:{w}px;height:{h}px;border-radius:8px;background:{a["plate"]};box-shadow:0 8px 14px rgba(0,0,0,.65),inset 0 0 0 3px {a["trim"]}"></div>']
    cap_ink = f'color:{ink}'
    if not big:
        ph = 150
        pos = [(27 + 55 * i, 18) for i in range(6)]; r = 15; jx, jy = 16, 52; line_y = 124; lamp = (w - 30, 52)
    else:
        ph = ph_big; top0 = 36 if ph >= 290 else 24; step = 112 if ph >= 290 else 104
        pos = [(w // 2 - 34 + (i % 3 - 1) * 110, top0 + (i // 3) * step) for i in range(6)]; r = 22; jx, jy = 22, top0 + 34 - 4; line_y = ph - 28; lamp = (w - 30, 40)
    s.append(div(6, 6, w - 12, ph, style=f'border-radius:5px;background:linear-gradient({shade(a["plate"],.92)},{shade(a["plate"],.8)});box-shadow:inset 0 0 0 1px {a["trim"]}'))
    labels = ['GAIN', 'BASS', 'MID', 'TREBLE', 'PRES', 'MASTER']
    for i, (kx, ky) in enumerate(pos):
        s.append(knob(6 + kx, 6 + ky, r, labels[i], '5.0', rot=-50 + 20 * i).replace('class="a cap" style="', f'class="a cap" style="color:{ink};'))
    s.append(div(6 + jx - 10, 6 + jy - 10, 20, 20, style='border-radius:50%;background:radial-gradient(circle,#111 35%,#aaa 40%,#555);', c=True))
    s.append(div(6 + jx - 12, 6 + jy - 34 if not big else 6 + jy - 34, 24, 14, 'cap', inner='IN', style=f'font-size:13px;line-height:13px;{cap_ink}', c=True))
    s.append(div(6 + 42, 6 + line_y - 11, 76, 22, 'cap', inner=f'AMP {num}', style=f'font-size:18px;line-height:22px;{cap_ink}', c=True))
    s.append(div(6 + lamp[0] - 7, 6 + lamp[1] - 7, 14, 14, 'led on', c=True))
    gy = 6 + ph + 8
    s.append(div(14, gy, w - 28, h - gy - 14, style=f'border-radius:4px;background:{grille_cloth("#6b5a2e")};box-shadow:inset 0 0 0 2px {a["trim"]}'))
    nh = 34
    s.append(div(w // 2 - 80, gy + (h - gy - 14 - nh) // 2, 160, nh, style=f'background:#0c0b0a;border:2px solid {a["trim"]};border-radius:4px;display:flex;align-items:center;justify-content:space-between;padding:0 10px;font-family:Bowlby;font-size:19px;color:#eadfc8', inner=f'<span>{a["name"]}</span><span style="font-size:13px">&#9662;</span>', c=True))
    return div(x, y, w, h, inner=''.join(s))

def cab(x, y, w, h, which, speakers=1):
    a = HEAD[which]
    s = [f'<div class="a" style="left:0;top:0;width:{w}px;height:{h}px;border-radius:6px;background:{a["cab"]};box-shadow:0 8px 14px rgba(0,0,0,.65),inset 0 0 0 3px {a["trim"]}"></div>']
    s.append(f'<div class="a" style="left:14px;top:14px;width:{w-28}px;height:{h-28-30}px;border-radius:4px;background:{grille_cloth()}"></div>')
    d = 96 if w < 230 else 100
    if speakers == 1:
        spots = [(w // 2, 14 + (h - 72) // 2)]
    else:
        cy1 = 14 + (h - 72) * 0.25; cy2 = 14 + (h - 72) * 0.75
        spots = [(w * 0.3, cy1), (w * 0.7, cy1), (w * 0.3, cy2), (w * 0.7, cy2)]
        d = 80
    for sx, sy in spots:
        s.append(f'<div class="a" style="left:{sx-d/2}px;top:{sy-d/2}px;width:{d}px;height:{d}px;border-radius:50%;background:radial-gradient(circle,#111 0 22%,#1f1d1b 24% 60%,#2e2b28 62%);box-shadow:0 0 0 3px #0c0b0a"></div>')
    s.append(div(w // 2 - 64, h - 28 - 20, 128, 22, 'chrome', style='display:flex;align-items:center;justify-content:space-between;padding:0 6px;font-family:Plex;font-size:13px;color:#222', inner=f'<span>&#9664;</span><span>{"OWN" if True else ""}</span><span>&#9654;</span>', c=True))
    return div(x, y, w, h, inner=''.join(s))

def coupler(x, y):
    return div(x, y - 4, 12, 8, 'chrome', c=False)

def ghost_slot(x, y, w, h):
    s = [div(0, 0, w, h, 'ghost')]
    s.append(head_glyph(w // 2 - 22, h // 2 - 70, False))
    s.append(div(w // 2 - 120, h // 2 - 22, 240, 44, 'add', inner='<span style="font-size:34px;line-height:30px">+</span><span>ADD SECOND AMP</span>', c=True))
    s.append(div(w // 2 - 150, h // 2 + 34, 300, 15, 'tag', inner='SAME AS THE AMPS ICON ON THE PLATE', style='text-align:center', c=True))
    return div(x, y, w, h, inner=''.join(s))

def context(wh, pedal_top):
    s = []
    s.append(f'<div class="a" style="left:0;top:10px;width:{CW}px;height:64px;background:linear-gradient(#a02024,#7a1519 60%,#5c0f12);box-shadow:0 6px 12px rgba(0,0,0,.6)"></div>')
    s.append(f'<div class="a" style="left:{CW//2-80}px;top:28px;width:160px;height:28px;white-space:nowrap;background:#0c0b0a;border-radius:5px;font-family:Bowlby;font-size:19px;line-height:28px;text-align:center;color:#eadfc8">GUITAR AMP</div>')
    s.append(f'<div data-ctx="1">' + g.pedal(OX - 207, pedal_top, 180, 310, 'drive', r=17) + g.pedal(OX + 790 + 27, pedal_top, 180, 310, 'delay', r=17) + '</div>')
    return ''.join(s)

def page(name, two, direction):
    # per direction: sizes
    wh = WIN_H_FULL
    sh = 490
    if direction == 'C' and not two:
        sh = 340; wh = STACK_TOP + sh + 10
    pedal_top = wh - 10 - 310
    parts = [context(wh, pedal_top)]
    oy = STACK_TOP
    plug_y = pedal_top + 282
    cabs_inner = []
    out_pos = None
    plate_w = 148
    if two:
        parts.append(head(OX, oy, 420, 236, 'torden', 1)); parts.append(cab(OX + 426, oy, 210, 236, 'torden', 4))
        parts.append(head(OX, oy + 254, 420, 236, 'klar', 2)); parts.append(cab(OX + 426, oy + 254, 210, 236, 'klar', 1))
        p, out = plate(OX + 642, oy, 148, 490, True)
        parts.append(p)
        j1 = (OX + 22, oy + 58); j2 = (OX + 22, oy + 254 + 58)
        cab_svg = [cable([(OX - 27, plug_y), (OX - 27 + 22, plug_y), (OX - 5, j2[1] + 6), j2]),
                   cable([(j2[0], j2[1] - 4), (OX - 28, j2[1] - 30), (OX - 28, j1[1] + 30), j1])]
        couplers = [(OX + 636, oy + 118), (OX + 636, oy + 254 + 118)]
    else:
        if direction == 'A':
            parts.append(head(OX, oy, 420, 236, 'torden', 1)); parts.append(cab(OX + 426, oy, 210, 236, 'torden', 4))
            parts.append(ghost_slot(OX, oy + 254, 636, 236))
            p, out = plate(OX + 642, oy, 148, 490, False, blend=False)
            couplers = [(OX + 636, oy + 118)]
        elif direction == 'B':
            parts.append(head(OX, oy, 420, 490, 'torden', 1, big=True)); parts.append(cab(OX + 426, oy, 238, 490, 'torden', 4))
            p, out = plate(OX + 666, oy, 124, 490, False, blend=False)
            couplers = [(OX + 664, oy + 245)]
        else:
            parts.append(head(OX, oy, 420, sh, 'torden', 1, big=True, ph_big=250)); parts.append(cab(OX + 426, oy, 210, sh, 'torden', 4))
            p, out = plate_c(OX + 642, oy, 148, sh)
            couplers = [(OX + 636, oy + sh // 2)]
        parts.append(p)
        j1 = (OX + (22 if direction != 'A' else 22), oy + (58 if direction == 'A' else 6 + 36 + 34 - 4 + 6))
        if direction != 'A': j1 = (OX + 28, oy + 6 + (66 if direction == 'B' else 54))
        cab_svg = [cable([(OX - 27, plug_y), (OX - 27 + 22, plug_y), (OX - 8, j1[1] + 8), j1])]
    for cx_, cy_ in couplers:
        parts.append(coupler(cx_, cy_))
    svg_ = f'<svg class="a" style="left:0;top:0" width="{CW}" height="{wh}">{"".join(cab_svg)}</svg>'
    body = f'<body data-w="{CW}" data-h="{wh}" style="width:{CW}px;height:{wh}px">' + ''.join(parts) + svg_ + '</body>'
    html_ = f'<!doctype html><html><head><meta charset="utf-8"><style>{g.CSS}{EXTRA}body{{width:{CW}px;height:{wh}px}}</style></head>{body}</html>'
    open(os.path.join(OUT, name + '.html'), 'w').write(html_)

def plate_c(x, y, w, h):
    """C: the one-amp plate on a 340 high stack: the Cabinets frame is shortened so the jack stays at the pedals' plug height."""
    cx = w // 2
    s = [f'<div class="a" style="left:0;top:0;width:{w}px;height:{h}px;border-radius:6px;background:linear-gradient(#1c1b1a,#0e0d0d);border:2px solid #8a8a88;box-shadow:0 8px 24px rgba(0,0,0,.7)"></div>']
    for sx, sy in ((6, 6), (w - 15, 6), (6, h - 15), (w - 15, h - 15)):
        s.append(f'<div class="a screw" style="left:{sx}px;top:{sy}px"></div>')
    s.append(amps_icon(10, 10, w - 20, False))
    s.append(div(10, 84, w - 20, 188, 'silk'))
    s.append(div(cx - 40, 76, 80, 16, 'silkt', inner='CABINETS', c=True))
    s.append(knob(cx - 31 - 27, 96, 15, 'MIC', '5.0', rot=-20))
    s.append(knob(cx + 31 - 27, 96, 15, 'RES', '5.0', rot=20))
    s.append(div(cx - 40, 196, 80, 15, 'cap', inner='MIC TYPE', style='font-size:14px', c=True))
    s.append(div(cx - 22, 214, 44, 20, 'tape', inner='57', c=True))
    s.append(div(cx - 30 - 25, 242, 50, 15, 'cap', inner='CABS', style='font-size:14px', c=True))
    s.append(div(cx - 30 - 6, 262 - 2, 12, 12, 'led on', c=True))
    s.append(div(cx + 22 - 10, 255, 20, 20, 'foot', c=True))
    s.append(div(cx + 16 - 10, 302, 20, 20, style='border-radius:50%;background:radial-gradient(circle,#111 35%,#aaa 40%,#555);', c=True))
    s.append(div(cx + 16 - 22 - 36, 305, 36, 14, 'cap', inner='OUT', style='font-size:15px;line-height:14px', c=True))
    return div(x, y, w, h, inner=''.join(s)), (x + cx + 16, y + 312)

if __name__ == '__main__':
    for d in 'ABC':
        for two in (False, True):
            page(f'{d}-{"two" if two else "one"}', two, d)
