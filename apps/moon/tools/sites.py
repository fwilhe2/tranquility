#!/usr/bin/env python3
"""Cuts the real places' windows out of NASA's global maps.

For each site in SITES: a square of ground centred on it, N × N samples at
SPACING metres (x east, z south, the site in the middle), resampled from

- the Moon: LOLA's gridded heights (LDEM_512, the Lunar Orbiter Laser
  Altimeter alone, 512 pixels per degree, 59 m), and the Clementine 750 nm
  mosaic for albedo (118 m);
- Mars: MOLA's gridded heights (MEGDR, the Mars Orbiter Laser Altimeter,
  128 pixels per degree, 463 m), and the Viking MDIM 2.1 colour mosaic
  (232 m).

Only public-domain sources: every one of these is the work of NASA or the
USGS (U.S. government works). Merged products that carry other agencies'
terms (SLDEM2015 with JAXA's Kaguya, the HRSC blend with ESA's Mars
Express) are deliberately not used. See data/sites/SOURCES.md.

and written to apps/moon/data/sites/<id>.png (heights, 16-bit grey, as
steps of `quantum` metres above `floor`) and <id>.colour.png (half the
resolution), with a line per site in sites.txt:

    id floor quantum spacing n latitude longitude resolution

where resolution is the source's own sample spacing, m: what is finer
than that the app draws from the body's procedural ground.

Only the bytes a window needs are read, by HTTP range requests on the
raw files (PDS images and uncompressed BigTIFFs); nothing global is
downloaded. Needs Python 3 and Pillow; run from anywhere:

    python3 apps/moon/tools/sites.py            # every site
    python3 apps/moon/tools/sites.py hadley     # one
"""

import concurrent.futures
import http.client
import math
import os
import struct
import sys
import threading
import urllib.parse
import urllib.request

from PIL import Image

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "data", "sites")

# id, body, latitude, east longitude, spacing (m), samples on a side.
SITES = [
    ("tranquility", "moon", 0.67408, 23.47297, 60.0, 768),
    ("hadley", "moon", 26.13224, 3.63400, 60.0, 768),
    ("taurus", "moon", 20.19080, 30.77168, 60.0, 768),
    ("copernicus", "moon", 9.62, 339.92, 150.0, 768),
    ("vonkarman", "moon", -45.4446, 177.5991, 60.0, 768),
    ("jezero", "mars", 18.4447, 77.4508, 200.0, 768),
    ("gale", "mars", -4.95, 137.60, 200.0, 768),
    ("olympus", "mars", 18.40, 226.20, 200.0, 768),
    ("valles", "mars", -13.40, 298.60, 200.0, 768),
    ("endeavour", "mars", -2.28, 354.77, 200.0, 768),
]

MOON_R = 1737400.0
MARS_R = 3396190.0

LOLA = "https://pds-geosciences.wustl.edu/lro/lro-l-lola-3-rdr-v1/lrolol_1xxx/data/lola_gdr/cylindrical/float_img/"
MOLA = "https://pds-geosciences.wustl.edu/mgs/mgs-m-mola-5-megdr-l3-v1/mgsl_300x/meg128/"
USGS = "https://planetarymaps.usgs.gov/mosaic/"

local = threading.local()


AGENT = {"User-Agent": "tranquility-sites/1 (python)"}


def resolve(url):
    """Follows redirects once, to the host that serves the bytes."""
    req = urllib.request.Request(url, headers={**AGENT, "Range": "bytes=0-0"})
    with urllib.request.urlopen(req, timeout=60) as r:
        return r.geturl()


def fetch(url, start, end):
    """Bytes start..end (inclusive) of a remote file, on a kept-alive
    connection per thread."""
    u = urllib.parse.urlsplit(url)
    for attempt in range(5):
        conn = getattr(local, "conns", {}).get(u.netloc)
        if conn is None:
            conn = http.client.HTTPSConnection(u.netloc, timeout=60)
            local.conns = getattr(local, "conns", {})
            local.conns[u.netloc] = conn
        try:
            path = u.path + (("?" + u.query) if u.query else "")
            conn.request("GET", path, headers={**AGENT, "Range": f"bytes={start}-{end}"})
            r = conn.getresponse()
            data = r.read()
            if r.status != 206:
                raise IOError(f"{url}: HTTP {r.status}")
            return data
        except (OSError, http.client.HTTPException):
            conn.close()
            local.conns.pop(u.netloc, None)
            if attempt == 4:
                raise
    raise IOError(url)


class Pds:
    """One tile of a PDS gridded product: a raw grid of samples (`fmt`, a
    struct code with its byte order), times `scale` to metres."""

    def __init__(self, url, lat_n, lon_w, ppd, lines, samples, fmt, scale):
        self.url, self.lat_n, self.lon_w, self.ppd = url, lat_n, lon_w, ppd
        self.lines, self.samples = lines, samples
        self.fmt, self.scale = fmt, scale

    def pixel(self, lat, lon):
        return (self.lat_n - lat) * self.ppd - 0.5, ((lon - self.lon_w) % 360.0) * self.ppd - 0.5

    def span(self, row, c0, c1):
        size = struct.calcsize(self.fmt)
        off = (row * self.samples + c0) * size
        data = fetch(self.url, off, off + (c1 - c0 + 1) * size - 1)
        n = c1 - c0 + 1
        return [[v * self.scale for v in struct.unpack(self.fmt[0] + self.fmt[1:] * n, data)]]


class Tiff:
    """A global map as an uncompressed BigTIFF, one row to a strip: read
    by rows, georeferenced from its tie point and pixel scale."""

    def __init__(self, url, radius):
        self.url = resolve(url)
        head = fetch(self.url, 0, 65535)
        bo = "<" if head[:2] == b"II" else ">"
        assert struct.unpack(bo + "H", head[2:4])[0] == 43, "BigTIFF expected"
        off = struct.unpack(bo + "Q", head[8:16])[0]
        n = struct.unpack(bo + "Q", head[off:off + 8])[0]
        tags = {}
        for i in range(n):
            e = head[off + 8 + i * 20: off + 28 + i * 20]
            tag, typ, cnt = struct.unpack(bo + "HHQ", e[:12])
            tags[tag] = (typ, cnt, e[12:20])
        size = {1: 1, 2: 1, 3: 2, 4: 4, 12: 8, 16: 8}

        def values(tag):
            typ, cnt, raw = tags[tag]
            fmt = {1: "B", 2: "c", 3: "H", 4: "I", 12: "d", 16: "Q"}[typ]
            nbytes = size[typ] * cnt
            if nbytes <= 8:
                return list(struct.unpack(bo + fmt * cnt, raw[:nbytes]))
            at = struct.unpack(bo + "Q", raw)[0]
            data = fetch(self.url, at, at + nbytes - 1)
            return list(struct.unpack(bo + fmt * cnt, data))

        self.bo = bo
        self.width, self.height = values(256)[0], values(257)[0]
        self.lines, self.samples = self.height, self.width
        assert values(259)[0] == 1, "uncompressed expected"
        assert values(278)[0] == 1, "a row to a strip expected"
        self.bits = values(258)[0]
        self.bands = values(277)[0] if 277 in tags else 1
        self.planar = values(284)[0] if 284 in tags else 1
        self.signed = (values(339)[0] if 339 in tags else 1) == 2
        self.offsets = values(273)
        sx, sy, _ = values(33550)
        i, j, _, x0, y0, _ = values(33922)
        self.sx, self.sy = sx, sy
        self.x0, self.y0 = x0 - i * sx, y0 + j * sy
        self.radius = radius
        # Map units: degrees (a geographic grid) or metres on the sphere.
        self.degree = 1.0 if abs(sx) < 1.0 else radius * math.pi / 180.0

    def pixel(self, lat, lon):
        lon = (lon + 180.0) % 360.0 - 180.0
        x, y = lon * self.degree, lat * self.degree
        return (self.y0 - y) / self.sy - 0.5, (x - self.x0) / self.sx - 0.5

    def span(self, row, c0, c1):
        b = self.bits // 8
        fmt = {(1, False): "B", (2, True): "h", (2, False): "H"}[(b, self.signed)]
        out = []
        if self.planar == 2:
            for band in range(self.bands):
                at = self.offsets[band * self.height + row] + c0 * b
                data = fetch(self.url, at, at + (c1 - c0 + 1) * b - 1)
                out.append(list(struct.unpack(self.bo + fmt * (c1 - c0 + 1), data)))
        else:
            at = self.offsets[row] + c0 * b * self.bands
            data = fetch(self.url, at, at + (c1 - c0 + 1) * b * self.bands - 1)
            flat = struct.unpack(self.bo + fmt * ((c1 - c0 + 1) * self.bands), data)
            out = [list(flat[k::self.bands]) for k in range(self.bands)]
        return out


tiles = {}


def lola(lat, lon):
    """The LOLA LDEM_512 tile (90° by 45°, floats in km) a point lies in."""
    lon = lon % 360.0
    band = math.floor(lat / 45.0) * 45
    west = math.floor(lon / 90.0) * 90
    names = {0: "00n_45n", 45: "45n_90n", -45: "45s_00s", -90: "90s_45s"}
    name = f"ldem_512_{names[band]}_{west:03d}_{west + 90:03d}_float.img"
    if name not in tiles:
        tiles[name] = Pds(LOLA + name, band + 45, west, 512, 23040, 46080, "<f", 1000.0)
    return tiles[name]


def mola(lat, lon):
    """The MOLA MEGDR tile (90° by 44°, 16-bit metres above the areoid,
    big-endian) a point lies in; tiles are named by their top latitude."""
    lon = lon % 360.0
    top = (math.floor(lat / 44.0) + 1) * 44
    west = math.floor(lon / 90.0) * 90
    tag = f"{abs(top):02d}{'n' if top >= 0 else 's'}"
    name = f"megt{tag}{west:03d}hb.img"
    if name not in tiles:
        tiles[name] = Pds(MOLA + name, top, west, 128, 5632, 11520, ">h", 1.0)
    return tiles[name]


def grid(lat0, lon0, spacing, n, radius):
    """Latitude and longitude of each sample of the window: x east and z
    south of the site, an equirectangular square about it."""
    out = []
    for j in range(n):
        z = (j - (n - 1) / 2.0) * spacing
        row = []
        for i in range(n):
            x = (i - (n - 1) / 2.0) * spacing
            lat = lat0 - math.degrees(z / radius)
            lon = lon0 + math.degrees(x / (radius * math.cos(math.radians(lat0))))
            row.append((lat, lon))
        out.append(row)
    return out


def cubic(a, b, c, d, t):
    """Catmull-Rom between b and c."""
    return b + 0.5 * t * (c - a + t * (2 * a - 5 * b + 4 * c - d + t * (3 * (b - c) + d - a)))


def sample(points, source_of, bands):
    """Bicubic samples of a map at many points, so that a coarse map
    resampled finer has no creases along its own grid: first every row span
    the points need is fetched (in parallel), then they are read."""
    need = {}
    where = []
    for row in points:
        r_out = []
        for lat, lon in row:
            src = source_of(lat, lon)
            fr, fc = src.pixel(lat, lon)
            # At a tile's edge its last rows and columns stand in for the
            # next tile's first.
            fr = min(max(fr, 1.0), src.lines - 2.001)
            fc = min(max(fc, 1.0), src.samples - 2.001)
            r0, c0 = math.floor(fr), math.floor(fc)
            for rr in range(r0 - 1, r0 + 3):
                lo, hi = need.get((src, rr), (c0 - 1, c0 + 2))
                need[(src, rr)] = (min(lo, c0 - 1), max(hi, c0 + 2))
            r_out.append((src, fr, fc))
        where.append(r_out)
    rows = {}

    def get(key):
        src, rr = key
        lo, hi = need[key]
        return key, lo, src.span(rr, lo, hi)

    total = len(need)
    with concurrent.futures.ThreadPoolExecutor(16) as pool:
        for k, (key, lo, data) in enumerate(pool.map(get, need)):
            rows[key] = (lo, data)
            if k % 200 == 0:
                print(f"  rows {k}/{total}", file=sys.stderr)
    out = []
    for r_out in where:
        line = []
        for src, fr, fc in r_out:
            r0, c0 = math.floor(fr), math.floor(fc)
            tr, tc = fr - r0, fc - c0
            vals = []
            for b in range(bands):
                def at(rr, cc):
                    lo, data = rows[(src, rr)]
                    return data[b][cc - lo]
                across = [cubic(*(at(rr, cc) for cc in range(c0 - 1, c0 + 3)), tc) for rr in range(r0 - 1, r0 + 3)]
                vals.append(cubic(*across, tr))
            line.append(vals)
        out.append(line)
    return out


def cut(site):
    sid, body, lat, lon, spacing, n = site
    radius = MOON_R if body == "moon" else MARS_R
    print(f"{sid}: {n}² at {spacing:.0f} m", file=sys.stderr)
    pts = grid(lat, lon, spacing, n, radius)
    if body == "moon":
        heights = sample(pts, lola, 1)
        colour_src = Tiff(USGS + "Lunar_Clementine_UVVIS_750nm_Global_Mosaic_118m_v2.tif", MOON_R)
    else:
        heights = sample(pts, mola, 1)
        colour_src = Tiff(USGS + "Mars_Viking_MDIM21_ClrMosaic_global_232m.tif", MARS_R)
    flat = [v[0] for row in heights for v in row]
    floor, top = min(flat), max(flat)
    quantum = max((top - floor) / 65000.0, 0.02)
    data = bytearray()
    for h in flat:
        data += struct.pack("<H", min(65535, round((h - floor) / quantum)))
    Image.frombytes("I;16", (n, n), bytes(data)).save(os.path.join(OUT, f"{sid}.png"), optimize=True)
    # Colour at half the resolution: the maps are coarser than the ground.
    half = grid(lat, lon, spacing * 2.0, n // 2, radius)
    colour = sample(half, lambda la, lo: colour_src, colour_src.bands)
    mode = "L" if colour_src.bands == 1 else "RGB"
    pixels = bytearray()
    for row in colour:
        for v in row:
            pixels += bytes(max(0, min(255, round(c))) for c in v)
    Image.frombytes(mode, (n // 2, n // 2), bytes(pixels)).save(os.path.join(OUT, f"{sid}.colour.png"), optimize=True)
    centre = heights[n // 2][n // 2][0]
    print(f"{sid}: {floor:.0f} to {top:.0f} m, the site at {centre:.0f} m", file=sys.stderr)
    resolution = 59.2 if body == "moon" else 463.0
    return f"{sid} {floor:.3f} {quantum:.6f} {spacing:.1f} {n} {lat} {lon} {resolution}"


def main():
    os.makedirs(OUT, exist_ok=True)
    wanted = set(sys.argv[1:])
    index = os.path.join(OUT, "sites.txt")
    lines = {}
    if os.path.exists(index):
        for line in open(index):
            if line.strip():
                lines[line.split()[0]] = line.strip()
    for site in SITES:
        if wanted and site[0] not in wanted:
            continue
        lines[site[0]] = cut(site)
    with open(index, "w") as f:
        for site in SITES:
            if site[0] in lines:
                f.write(lines[site[0]] + "\n")


if __name__ == "__main__":
    main()
