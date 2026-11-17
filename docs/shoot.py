"""Renders the README's pictures into docs/screenshots with headless Chrome (or Edge), each in a dark
and a light version: the banner, the card and its back, the characters' moods, a question round.

    python docs/shoot.py

The pages are the app's own HTML on made-up data (docs/mock.html), so nothing of yours shows up.
Needs Pillow for trimming the transparent edges.
"""
import os, shutil, subprocess, sys, tempfile
from pathlib import Path
from urllib.parse import quote

from PIL import Image

DOCS = Path(__file__).resolve().parent
OUT = DOCS / "screenshots"
NAME = sys.argv[1] if len(sys.argv) > 1 else "Meownitor"

CANDIDATES = [
    r"C:\Program Files\Google\Chrome\Application\chrome.exe",
    r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "google-chrome",
]
BROWSER = next((c for c in CANDIDATES if shutil.which(c) or os.path.exists(c)), None)


def shot(url, out, w, h, trim=False):
    args = [BROWSER, "--headless=new", "--disable-gpu", "--hide-scrollbars", "--allow-file-access-from-files",
            "--force-device-scale-factor=2", "--default-background-color=00000000", f"--window-size={w},{h}",
            "--virtual-time-budget=2500", f"--screenshot={out}", url]
    subprocess.run(args, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    if trim:
        im = Image.open(out).convert("RGBA")
        box = im.getchannel("A").getbbox()
        if box:
            pad = 16
            im.crop((max(0, box[0] - pad), max(0, box[1] - pad), min(im.width, box[2] + pad), min(im.height, box[3] + pad))).save(out)
    print(out.relative_to(DOCS.parent))


def page(name, query):
    return (DOCS / name).as_uri() + "?" + query


def round_page(theme, tmp):
    # The question page the skill starts from, in the kit's look, with the first option picked.
    kit = DOCS.parent / "skill" / "meownitor-round"
    for f in ("round-kit.css", "round-kit.js"):
        shutil.copy(kit / f, tmp / f)
    html = (kit / "template-choice.html").read_text(encoding="utf-8")
    html = html.replace('<html lang="en">', f'<html lang="en" data-theme="{theme}">', 1)
    html = html.replace("</body>", "<script>addEventListener('load',function(){var v=document.querySelector('.var[data-v=B]');if(v)v.click();});</script></body>", 1)
    p = tmp / f"round-{theme}.html"
    p.write_text(html, encoding="utf-8")
    return p.as_uri()


def main():
    if not BROWSER:
        sys.exit("No Chrome or Edge found")
    OUT.mkdir(exist_ok=True)
    n = quote(NAME)
    with tempfile.TemporaryDirectory() as t:
        tmp = Path(t)
        for th in ("dark", "light"):
            shot(page("hero.html", f"theme={th}&name={n}"), OUT / f"hero-{th}.png", 1280, 640)
            shot(page("mock.html", f"bg=none&how=card&mood=work&theme={th}&name={n}"), OUT / f"card-{th}.png", 360, 900, trim=True)
            shot(page("mock.html", f"bg=none&how=settings&theme={th}&name={n}"), OUT / f"settings-{th}.png", 360, 900, trim=True)
            shot(page("mock.html", f"bg=none&how=card&lang=uk&theme={th}&name={n}"), OUT / f"card-uk-{th}.png", 360, 900, trim=True)
            shot(page("moods.html", f"theme={th}"), OUT / f"moods-{th}.png", 900, 420, trim=True)
            shot(round_page(th, tmp), OUT / f"round-{th}.png", 1100, 900)


if __name__ == "__main__":
    main()
