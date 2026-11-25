from PIL import Image, ImageDraw, ImageFont
import os
D = os.path.dirname(os.path.abspath(__file__))
font = ImageFont.truetype(os.path.join(D, '../../crates/suite_common/assets/fonts/IBMPlexMono-Medium.ttf'), 20)
names = {'A': 'A  Empty slot', 'B': 'B  Grown lane', 'C': 'C  Short window'}
for d in 'ABC':
    ims = [Image.open(os.path.join(D, f'{d}-{m}.png')).convert('RGB') for m in ('one', 'two')]
    H = sum(i.height for i in ims) + 2 * 40 + 10
    s = Image.new('RGB', (1100, H), '#0a0a0a'); dr = ImageDraw.Draw(s); y = 0
    for i, m in zip(ims, ('one amp', 'two amps')):
        dr.text((12, y + 8), f'{names[d]} - {m} ({i.width}x{i.height})', fill='#e8c46a', font=font); y += 40
        s.paste(i, (0, y)); y += i.height + 5
    s.save(os.path.join(D, f'sheet-{d}.png'))
