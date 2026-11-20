#!/usr/bin/env python3
"""Makes the Earth's map for the Moon's sky (`src/heavens.rs`) from NASA's
Blue Marble Next Generation, December, with topography and bathymetry
(NASA Earth Observatory, Reto Stöckli; public domain): the land and sea as
they look in the season of Apollo 8, without clouds. The clouds are the
app's own, painted over it.

Writes apps/moon/data/earth/earth.png, 2048 × 1024, equirectangular from
180° W at the left edge, north up. Needs Python 3 and Pillow:

    python3 apps/moon/tools/earth.py
"""

import io
import os
import urllib.request

from PIL import Image

URL = ("https://assets.science.nasa.gov/content/dam/science/esd/eo/images/bmng/"
       "bmng-topography-bathymetry/december/world.topo.bathy.200412.3x5400x2700.jpg")
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "data", "earth", "earth.png")


def main():
    req = urllib.request.Request(URL, headers={"User-Agent": "tranquility-earth/1 (python)"})
    data = urllib.request.urlopen(req, timeout=300).read()
    image = Image.open(io.BytesIO(data)).convert("RGB")
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    image.resize((2048, 1024), Image.LANCZOS).save(OUT, optimize=True)
    print(f"{OUT}: {os.path.getsize(OUT)} bytes")


if __name__ == "__main__":
    main()
