"""Rebuilds branding/index.html from the SVG files. Run: python3 branding/tools/build_board.py"""
import html
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
INK, PAPER, ORANGE, PEACH = '#2A2723', '#FCFBF8', '#C2410C', '#F2B48E'

DIRECTIONS = [
    dict(dir='01-pulse', num='01', name='Pulse', concept='The listening pill becomes the logo: five bars whose peaks also trace an “M”.',
         en='Speak. It’s written.', nl='Spreek. Het staat er.', accent=ORANGE, wave_mark=True),
    dict(dir='02-bowl', num='02', name='Bowl', concept='A bowl of three grains, read as an ellipsis: everything you said, mixed and served.',
         en='Everything you said, nicely served.', nl='Alles wat je zei, netjes opgediend.', accent=ORANGE),
    dict(dir='03-folio', num='03', name='Folio', concept='A sheet of paper that is also a speech bubble: voice in, page out.',
         en='From voice to page.', nl='Van stem naar pagina.', accent=ORANGE),
    dict(dir='04-line', num='04', name='Line', concept='A single-stroke “m” whose last leg runs out into a line of text, ending at the cursor.',
         en='Talk in. Text out.', nl='Praat erin. Tekst eruit.', accent=ORANGE),
    dict(dir='05-quote', num='05', name='Quote', concept='Two grains shaped as a closing quotation mark: your words and theirs, quoted faithfully.',
         en='Quote yourself. And the whole room.', nl='Citeer jezelf. En de hele tafel.', accent=ORANGE),
    dict(dir='06-full-stop', num='06', name='Full Stop', concept='Wordmark-led: an editorial serif “musli” closed by an orange full stop. Spoken, then finished.',
         en='Say it. Full stop.', nl='Zeg het. Punt uit.', accent=ORANGE),
]
ALTERNATIVES = [
    dict(dir='alt-zinly', num='A', name='Zinly', concept='“zin” is Dutch for sentence: a “z” drawn as two lines of text joined by one stroke.',
         en='Every sentence, sorted.', nl='Elke zin, op z’n plek.', accent=ORANGE, brand='Zinly'),
    dict(dir='alt-saywell', num='B', name='Saywell', concept='“Say it well”: a round speech bubble holding the three-bar listening wave.',
         en='Say it once. Say it well.', nl='Zeg het één keer. Goed.', accent=ORANGE, brand='Saywell'),
]


def load(d, name):
    text = (ROOT / d / f'{name}.svg').read_text()
    text = re.sub(r'<!--.*?-->\s*', '', text, flags=re.S)
    return re.sub(r'\s(width|height)="\d+"', '', text, count=2)


def on_dark(svg):
    return svg.replace(INK, PAPER).replace(ORANGE, PEACH)


def template(svg):
    return svg.replace('#000', 'currentColor')


WAVE = ''.join(f'<i style="--h:{h};--d:{d}s"></i>' for h, d in [(.45, 0), (.8, .15), (1, .3), (.7, .45), (.4, .6)])
DOCK_APPS = [
    ('#4F7DF3', '<circle cx="512" cy="512" r="190" fill="none" stroke="#fff" stroke-width="56"/>'),
    ('#3FAE6A', '<rect x="330" y="360" width="364" height="304" rx="40" fill="#fff"/>'),
    ('#ECE7DE', '<path d="M340 680 512 330 684 680Z" fill="#9B948A"/>'),
    ('#F4C542', '<rect x="330" y="330" width="364" height="364" rx="60" fill="#fff"/><path d="M380 430H644M380 512H644M380 594H560" stroke="#E0B12E" stroke-width="36" stroke-linecap="round"/>'),
    ('#6B5BD6', '<circle cx="430" cy="512" r="90" fill="#fff"/><circle cx="600" cy="512" r="90" fill="#fff" opacity=".6"/>'),
]


def dock_tile(bg, inner):
    return f'<svg viewBox="0 0 1024 1024"><rect x="100" y="100" width="824" height="824" rx="185" fill="{bg}"/>{inner}</svg>'


def card(d):
    brand = d.get('brand', 'Musli')
    mark, word, icon, menu = (load(d['dir'], n) for n in ('mark', 'wordmark', 'app-icon', 'menubar'))
    pill_mark = '' if d.get('wave_mark') else f'<span class="pill-mark">{on_dark(mark)}</span>'
    docks = ''.join(f'<span class="tile">{dock_tile(bg, inner)}</span>' for bg, inner in DOCK_APPS[:3])
    docks += f'<span class="tile me">{icon}</span>'
    docks += ''.join(f'<span class="tile">{dock_tile(bg, inner)}</span>' for bg, inner in DOCK_APPS[3:])
    strip = lambda cls: f'''<div class="menubar {cls}"><span class="apple"></span><b>Finder</b><span>File</span><span>Edit</span>
        <span class="spacer"></span><span class="mb-icon">{template(menu)}</span><span class="batt"></span><span>Fri 24 Oct&nbsp; 09:41</span></div>'''
    return f'''
<section class="card" id="d{d["num"]}">
  <header><span class="num">{d["num"]}</span><div><h2>{html.escape(d["name"])}</h2><p>{d["concept"]}</p></div></header>
  <div class="stage">
    <div class="mark">{mark}</div>
    <div class="word">{word}</div>
  </div>
  <div class="label">App icon · 1024 master (shown 192) · 128 · 32 px</div>
  <div class="icons">
    <a class="i192" href="{d["dir"]}/app-icon.svg" title="Open the 1024 px master">{icon}</a>
    <span class="i128">{icon}</span><span class="i32">{icon}</span>
  </div>
  <div class="label">Menubar template icon · real 18 pt</div>
  {strip('light')}
  {strip('dark')}
  <div class="label">Dock</div>
  <div class="wallpaper"><div class="dock">{docks}</div></div>
  <div class="label">Dictation pill</div>
  <div class="pill-row"><div class="pill">{pill_mark}<span class="wave">{WAVE}</span></div><span class="hint">Hold <kbd>fn</kbd> to dictate</span></div>
  <div class="label">Landing hero</div>
  <div class="hero">
    <nav><span class="nav-word">{word}</span><span class="links">Features · Pricing · GitHub</span></nav>
    <h3>{d["en"]}</h3>
    <p class="nl">{d["nl"]}</p>
    <p class="sub">Dictation anywhere you type, and meeting notes that stay on your Mac. 13 MB. No account.</p>
    <div class="ctas"><span class="btn">Download for Mac</span><span class="btn ghost">Try {brand} Cloud</span></div>
  </div>
</section>'''


CSS = '''
@font-face{font-family:Figtree;src:url(../apps/macos/Resources/Fonts/Figtree-Regular.ttf);font-weight:400}
@font-face{font-family:Figtree;src:url(../apps/macos/Resources/Fonts/Figtree-Medium.ttf);font-weight:500}
@font-face{font-family:Figtree;src:url(../apps/macos/Resources/Fonts/Figtree-SemiBold.ttf);font-weight:600}
@font-face{font-family:"Instrument Serif";src:url(../apps/macos/Resources/Fonts/InstrumentSerif-Regular.ttf)}
:root{--paper:#FCFBF8;--bg:#F7F4EE;--ink:#2A2723;--muted:#9B948A;--line:#ECE7DE;--accent:#C2410C;--wave:#F2B48E}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--ink);font:15px/1.5 Figtree,system-ui,sans-serif;-webkit-font-smoothing:antialiased}
.wrap{max-width:1760px;margin:0 auto;padding:56px 40px 80px}
h1{font:400 64px/1 "Instrument Serif",serif;margin:0 0 12px;letter-spacing:-.01em}
h1 em{color:var(--accent)}
.lede{max-width:760px;color:#5c564e;margin:0 0 28px;font-size:17px}
.swatches{display:flex;gap:10px;flex-wrap:wrap;margin-bottom:24px}
.sw{display:flex;align-items:center;gap:8px;background:var(--paper);border:1px solid var(--line);border-radius:999px;padding:5px 12px 5px 5px;font-size:12px;color:#5c564e}
.sw i{width:22px;height:22px;border-radius:50%;border:1px solid rgba(0,0,0,.06)}
.reco{background:var(--paper);border:1px solid var(--line);border-left:3px solid var(--accent);border-radius:14px;padding:16px 20px;max-width:980px;margin-bottom:40px}
.reco b{font-weight:600}
.grid{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:28px}
@media (max-width:1400px){.grid{grid-template-columns:repeat(2,minmax(0,1fr))}}
@media (max-width:900px){.grid{grid-template-columns:1fr}}
.card{background:var(--paper);border:1px solid var(--line);border-radius:22px;padding:26px}
.card header{display:flex;gap:16px;align-items:flex-start;margin-bottom:18px}
.num{font:400 52px/0.9 "Instrument Serif",serif;color:var(--accent);min-width:56px}
.card h2{font:400 32px/1 "Instrument Serif",serif;margin:2px 0 6px}
.card header p{margin:0;color:#6b645a;font-size:14px}
.stage{display:flex;align-items:center;gap:28px;background:var(--bg);border-radius:14px;padding:22px 24px;min-height:140px}
.stage .mark svg{width:84px;height:84px;display:block}
.stage .word svg{height:60px;width:auto;max-width:100%;display:block}
.label{font-size:11px;letter-spacing:.08em;text-transform:uppercase;color:var(--muted);margin:22px 0 8px;font-weight:600}
.icons{display:flex;align-items:flex-end;gap:18px;background:linear-gradient(90deg,var(--bg) 0 50%,#d9d3c8 50% 100%);border-radius:14px;padding:12px 16px}
.icons svg{display:block;width:100%;height:100%}
.i192{width:192px;height:192px;display:block}.i128{width:128px;height:128px}.i32{width:32px;height:32px}
.menubar{display:flex;align-items:center;gap:14px;height:26px;padding:0 12px;font-size:13px;border-radius:8px;margin-bottom:6px}
.menubar.light{background:rgba(246,244,240,.95);color:#1d1d1f;border:1px solid var(--line)}
.menubar.dark{background:#2b2a29;color:#f2f2f2}
.menubar b{font-weight:600}.spacer{flex:1}
.apple{width:11px;height:13px;border-radius:50% 50% 45% 45%;background:currentColor;opacity:.85}
.mb-icon{width:18px;height:18px;display:inline-flex}.mb-icon svg{width:18px;height:18px}
.batt{width:22px;height:11px;border:1.5px solid currentColor;border-radius:3px;position:relative;opacity:.8}
.batt:after{content:"";position:absolute;inset:1.5px 6px 1.5px 1.5px;background:currentColor;border-radius:1px}
.wallpaper{height:116px;border-radius:14px;background:#B9AE9C;display:flex;align-items:flex-end;justify-content:center;padding-bottom:8px;overflow:hidden;position:relative}
.wallpaper:before{content:"";position:absolute;inset:0;background:radial-gradient(120% 90% at 20% 0%,#D8CDB8 0,transparent 60%),radial-gradient(90% 80% at 90% 10%,#8F8577 0,transparent 70%)}
.dock{position:relative;display:flex;gap:4px;padding:5px 8px;background:rgba(255,255,255,.38);border:1px solid rgba(255,255,255,.5);border-radius:20px;backdrop-filter:blur(18px)}
.tile{width:60px;height:60px;display:block;position:relative}.tile svg{width:100%;height:100%;display:block}
.tile.me:after{content:"";position:absolute;left:50%;bottom:-3px;width:4px;height:4px;margin-left:-2px;border-radius:50%;background:rgba(0,0,0,.55)}
.pill-row{display:flex;align-items:center;gap:16px}
.pill{display:inline-flex;align-items:center;gap:10px;background:var(--ink);border-radius:999px;padding:8px 16px;height:38px;box-shadow:0 6px 18px rgba(42,39,35,.18)}
.pill-mark svg{width:18px;height:18px;display:block}
.wave{display:flex;align-items:center;gap:3px;height:18px}
.wave i{display:block;width:3px;border-radius:2px;background:var(--paper);height:calc(var(--h)*18px);animation:w 1.2s ease-in-out var(--d) infinite}
.wave i:nth-child(3){background:var(--wave)}
@keyframes w{50%{transform:scaleY(.45)}}
.hint{font-size:13px;color:var(--muted)}
kbd{font:600 11px Figtree;border:1px solid var(--line);border-bottom-width:2px;border-radius:5px;padding:1px 6px;color:var(--ink);background:var(--paper)}
.hero{background:var(--bg);border-radius:14px;padding:16px 22px 24px;border:1px solid var(--line)}
.hero nav{display:flex;justify-content:space-between;align-items:center;margin-bottom:26px}
.nav-word svg{height:24px;width:auto;display:block}
.links{font-size:12px;color:var(--muted)}
.hero h3{font:400 40px/1.02 "Instrument Serif",serif;margin:0 0 6px;letter-spacing:-.01em}
.hero .nl{font:400 20px/1.2 "Instrument Serif",serif;font-style:italic;color:var(--accent);margin:0 0 12px}
.hero .sub{color:#5c564e;margin:0 0 16px;font-size:14px;max-width:420px}
.ctas{display:flex;gap:10px}
.btn{background:var(--ink);color:var(--paper);border-radius:999px;padding:8px 16px;font-size:13px;font-weight:600}
.btn.ghost{background:transparent;color:var(--ink);border:1px solid #d8d1c5}
.alts{margin-top:72px;padding-top:40px;border-top:1px solid var(--line)}
.alts h2.sec{font:400 44px/1 "Instrument Serif",serif;margin:0 0 10px}
.alts .grid{grid-template-columns:repeat(2,minmax(0,1fr))}
@media (max-width:900px){.alts .grid{grid-template-columns:1fr}}
footer{margin-top:56px;color:var(--muted);font-size:13px}
a{color:inherit}
'''

SWATCHES = [('Paper', '#FCFBF8'), ('Background', '#F7F4EE'), ('Ink', '#2A2723'), ('Muted', '#9B948A'),
            ('Line', '#ECE7DE'), ('Accent · burnt orange', '#C2410C'), ('Soft wave', '#F2B48E')]


def main():
    swatches = ''.join(f'<span class="sw"><i style="background:{c}"></i>{n} {c}</span>' for n, c in SWATCHES)
    page = f'''<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Musli · brand directions</title><style>{CSS}</style></head>
<body><div class="wrap">
<h1>Musli <em>·</em> six directions</h1>
<p class="lede">Dictation anywhere you type and meeting notes that stay yours. Six logo directions in the existing warm-paper language: Figtree for the interface, Instrument Serif for headings, one burnt-orange accent. Each card shows the mark, the lockup, the app icon from the 1024 master down to 32 px, the 18 pt menubar template on light and dark bars, the Dock, the listening pill and a landing hero.</p>
<div class="swatches">{swatches}</div>
<div class="reco"><b>Recommendation: 06 Full Stop</b> as the identity (wordmark, app icon, site), with <b>01 Pulse</b> as the menubar icon and listening state. “Zeg het. Punt uit.” works the same way in Dutch and English, the serif matches the headings already in the app, and the full stop says “finished, polished text”. The serif “m” is the weakest shape at 18 pt, and the bars are the strongest, so the menubar uses the bars. See <a href="palette.md">palette.md</a> and each folder’s notes.md.</div>
<div class="grid">{''.join(card(d) for d in DIRECTIONS)}</div>
<div class="alts">
  <h2 class="sec">If the name changes</h2>
  <p class="lede">“Musli” collides with Muesli (Muesli-HQ/muesli), an MIT-licensed Mac app for dictation and meeting notes, and the obvious domains are taken. These are the two strongest alternatives from <a href="names.md">names.md</a>, each as a quick mark and wordmark in the same visual system.</p>
  <div class="grid">{''.join(card(d) for d in ALTERNATIVES)}</div>
</div>
<footer>Generated from the SVG files by tools/build_board.py. Every SVG is hand-editable; wordmark text is outlined from the bundled fonts.</footer>
</div></body></html>'''
    (ROOT / 'index.html').write_text(page)
    print('wrote', ROOT / 'index.html')


if __name__ == '__main__':
    main()
