#!/usr/bin/env python3
"""Genera l'icona di Visualizzatore Immagini CP (assets/icon.ico e assets/icon.png).

Uso:  python tools/make_icon.py
Richiede Pillow (pip install pillow).
"""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFilter

S = 1024  # disegno a 1024 px, poi ridotto con LANCZOS
OUT = Path(__file__).resolve().parent.parent / "assets"


def lerp(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(len(a)))


def vgradient(size, top, bottom):
    w, h = size
    g = Image.new("RGBA", size)
    d = ImageDraw.Draw(g)
    for y in range(h):
        d.line([(0, y), (w, y)], fill=lerp(top, bottom, y / max(1, h - 1)) + (255,))
    return g


def rounded_mask(size, radius):
    m = Image.new("L", size, 0)
    ImageDraw.Draw(m).rounded_rectangle([0, 0, size[0] - 1, size[1] - 1], radius=radius, fill=255)
    return m


def main():
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))

    # sfondo: quadrato arrotondato con gradiente blu -> azzurro
    bg = vgradient((S, S), (37, 99, 235), (14, 165, 233))
    img.paste(bg, (0, 0), rounded_mask((S, S), 230))

    # scheda "dietro" (effetto pila di foto), leggermente ruotata
    back = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    ImageDraw.Draw(back).rounded_rectangle([250, 190, 850, 740], radius=70, fill=(255, 255, 255, 120))
    back = back.rotate(9, resample=Image.BICUBIC, center=(S // 2, S // 2))
    img.alpha_composite(back)

    # ombra della scheda principale
    shadow = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    ImageDraw.Draw(shadow).rounded_rectangle([170, 250, 830, 860], radius=80, fill=(8, 30, 90, 150))
    shadow = shadow.filter(ImageFilter.GaussianBlur(28))
    img.alpha_composite(shadow)

    # scheda principale bianca (cornice)
    card = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    ImageDraw.Draw(card).rounded_rectangle([160, 210, 820, 820], radius=80, fill=(255, 255, 255, 255))
    img.alpha_composite(card)

    # area immagine dentro la cornice
    x0, y0, x1, y1 = 205, 255, 775, 775
    w, h = x1 - x0, y1 - y0
    pic = vgradient((w, h), (125, 196, 255), (222, 242, 255))
    d = ImageDraw.Draw(pic)
    d.ellipse([w - 235, 50, w - 85, 200], fill=(255, 205, 64, 255))            # sole
    d.polygon([(0, h), (0, h - 190), (150, h - 330), (330, h - 120), (470, h - 230), (w, h - 90), (w, h)],
              fill=(30, 94, 190, 255))                                            # monti dietro
    d.polygon([(0, h), (0, h - 80), (180, h - 190), (330, h - 60), (460, h - 150), (w, h - 40), (w, h)],
              fill=(15, 60, 140, 255))                                            # monti davanti
    img.paste(pic, (x0, y0), rounded_mask((w, h), 48))

    # badge "CP" in basso a destra
    badge = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    bd = ImageDraw.Draw(badge)
    bd.ellipse([640, 640, 940, 940], fill=(255, 255, 255, 255))
    bd.ellipse([662, 662, 918, 918], fill=(16, 185, 129, 255))
    # lettere C e P disegnate con forme semplici (indipendenti dai font installati)
    bd.arc([690, 700, 780, 880], start=40, end=320, fill=(255, 255, 255, 255), width=24)     # C
    bd.rectangle([800, 702, 822, 878], fill=(255, 255, 255, 255))                              # P asta
    bd.arc([760, 698, 880, 800], start=270, end=90, fill=(255, 255, 255, 255), width=22)      # P pancia
    bd.line([(812, 704), (846, 704)], fill=(255, 255, 255, 255), width=22)
    bd.line([(812, 798), (846, 798)], fill=(255, 255, 255, 255), width=22)
    img.alpha_composite(badge)

    OUT.mkdir(exist_ok=True)
    img.resize((512, 512), Image.LANCZOS).save(OUT / "icon.png")
    sizes = [256, 128, 64, 48, 32, 24, 16]
    img.save(OUT / "icon.ico", format="ICO", sizes=[(s, s) for s in sizes])
    print("Creati:", OUT / "icon.png", OUT / "icon.ico")


if __name__ == "__main__":
    main()
