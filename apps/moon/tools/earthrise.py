#!/usr/bin/env python3
"""Works out the geometry of Apollo 8's Earthrise, for `src/earthrise.rs`.

From JPL's Horizons system (ssd.jpl.nasa.gov, the DE441 ephemeris) at the
moment of the colour photograph AS08-14-2383, 1968-12-24 16:39:39 UT
(75:48:39 ground elapsed time):

- the point on the Moon under the Sun and the point under the Earth
  (Moon mean-Earth frame, east longitude);
- the point on the Earth under the Moon and under the Sun, and the
  Earth's size seen from the Moon;

and from the mission's published numbers (the Apollo Flight Journal,
NASA's photo caption): an orbit 112 km up, inclined 12° and retrograde
(the spacecraft goes west), with the Earth some 5° above the horizon when
the photograph was taken, the view toward it crossing the crater Pasteur.

It prints the numbers `earthrise.rs` takes; they are copied there by hand
with this script named. Needs only Python 3:

    python3 apps/moon/tools/earthrise.py
"""

import math
import urllib.parse
import urllib.request

MOON_R = 1737.4
ALTITUDE = 112.0
INCLINATION = 12.0
PERIOD_MIN = 128.7
PHOTO = "1968-12-24 16:39:39"
# The middle of Pasteur, which the view toward the Earth crossed.
PASTEUR = (-11.9, 104.6)


def horizons(target, centre, quantities):
    q = {
        "format": "text",
        "COMMAND": f"'{target}'",
        "CENTER": f"'{centre}'",
        "MAKE_EPHEM": "YES",
        "EPHEM_TYPE": "OBSERVER",
        "TLIST": f"'{PHOTO}'",
        "TIME_TYPE": "UT",
        "QUANTITIES": f"'{quantities}'",
        "ANG_FORMAT": "DEG",
    }
    url = "https://ssd.jpl.nasa.gov/api/horizons.api?" + urllib.parse.urlencode(q)
    req = urllib.request.Request(url, headers={"User-Agent": "tranquility-earthrise/1 (python)"})
    text = urllib.request.urlopen(req, timeout=60).read().decode()
    row = text.split("$$SOE")[1].split("$$EOE")[0].split()
    # Date and time, then the numbers.
    return [float(v) for v in row[2:]]


def vec(lat, lon):
    la, lo = math.radians(lat), math.radians(lon)
    return (math.cos(la) * math.cos(lo), math.cos(la) * math.sin(lo), math.sin(la))


def latlon(v):
    return math.degrees(math.asin(v[2])), math.degrees(math.atan2(v[1], v[0])) % 360.0


def dot(a, b):
    return sum(x * y for x, y in zip(a, b))


def cross(a, b):
    return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])


def norm(a):
    n = math.sqrt(dot(a, a))
    return tuple(x / n for x in a)


def add(a, b, s=1.0):
    return tuple(x + s * y for x, y in zip(a, b))


def scale(a, s):
    return tuple(x * s for x in a)


def east_north(p):
    """The local east and north at a point on the sphere."""
    east = norm(cross((0.0, 0.0, 1.0), p))
    return east, cross(p, east)


def azimuth(p, d):
    """Degrees from north through east of a direction `d` seen from `p`."""
    east, north = east_north(p)
    return math.degrees(math.atan2(dot(d, east), dot(d, north))) % 360.0


def vectors(target):
    """Where a body is from the Moon's centre, km, ICRF axes (light time
    allowed for), and the Julian date."""
    q = {
        "format": "text",
        "COMMAND": f"'{target}'",
        "CENTER": "'500@301'",
        "MAKE_EPHEM": "YES",
        "EPHEM_TYPE": "VECTORS",
        "TLIST": f"'{PHOTO}'",
        "TIME_TYPE": "UT",
        "REF_SYSTEM": "ICRF",
        "REF_PLANE": "FRAME",
        "VEC_TABLE": "1",
        "VEC_CORR": "'LT'",
        "CSV_FORMAT": "YES",
    }
    url = "https://ssd.jpl.nasa.gov/api/horizons.api?" + urllib.parse.urlencode(q)
    req = urllib.request.Request(url, headers={"User-Agent": "tranquility-earthrise/1 (python)"})
    text = urllib.request.urlopen(req, timeout=60).read().decode()
    row = text.split("$$SOE")[1].split("$$EOE")[0].split(",")
    return float(row[0]), tuple(float(v) for v in row[2:5])


def moon_frame(jd):
    """ICRF to the Moon's body-fixed frame by the IAU's rotation model for
    the Moon (Archinal et al. 2011, the IAU WGCCRE 2009 report): a
    function taking a vector."""
    d = jd - 2451545.0
    t = d / 36525.0
    e = [0.0] + [math.radians(a + b * d) for a, b in [
        (125.045, -0.0529921), (250.089, -0.1059842), (260.008, 13.0120009), (176.625, 13.3407154),
        (357.529, 0.9856003), (311.589, 26.4057084), (134.963, 13.0649930), (276.617, 0.3287146),
        (34.226, 1.7484877), (15.134, -0.1589763), (119.743, 0.0036096), (239.961, 0.1643573),
        (25.053, 12.9590088)]]
    sin, cos = math.sin, math.cos
    a0 = (269.9949 + 0.0031 * t - 3.8787 * sin(e[1]) - 0.1204 * sin(e[2]) + 0.0700 * sin(e[3])
          - 0.0172 * sin(e[4]) + 0.0072 * sin(e[6]) - 0.0052 * sin(e[10]) + 0.0043 * sin(e[13]))
    d0 = (66.5392 + 0.0130 * t + 1.5419 * cos(e[1]) + 0.0239 * cos(e[2]) - 0.0278 * cos(e[3])
          + 0.0068 * cos(e[4]) - 0.0029 * cos(e[6]) + 0.0009 * cos(e[7]) + 0.0008 * cos(e[10])
          - 0.0009 * cos(e[13]))
    w = (38.3213 + 13.17635815 * d - 1.4e-12 * d * d + 3.5610 * sin(e[1]) + 0.1208 * sin(e[2])
         - 0.0642 * sin(e[3]) + 0.0158 * sin(e[4]) + 0.0252 * sin(e[5]) - 0.0066 * sin(e[6])
         - 0.0047 * sin(e[7]) - 0.0046 * sin(e[8]) + 0.0028 * sin(e[9]) + 0.0052 * sin(e[10])
         + 0.0040 * sin(e[11]) + 0.0019 * sin(e[12]) - 0.0044 * sin(e[13]))
    a0, d0, w = math.radians(a0), math.radians(d0), math.radians(w)

    def rz(a, v):
        return (cos(a) * v[0] + sin(a) * v[1], -sin(a) * v[0] + cos(a) * v[1], v[2])

    def rx(a, v):
        return (v[0], cos(a) * v[1] + sin(a) * v[2], -sin(a) * v[1] + cos(a) * v[2])

    return lambda v: rz(w, rx(math.pi / 2 - d0, rz(math.pi / 2 + a0, v)))


def local(at, v):
    """A direction in the frame of the ground at `at`: x east, y up, z
    south, as the app has it."""
    east, north = east_north(at)
    return (dot(v, east), dot(v, at), -dot(v, north))


def main():
    # Moon from the Earth: under the Earth, under the Sun.
    _, sub_lon, sub_lat, sun_lon, sun_lat = horizons("301", "500@399", "13,14,15")[:5]
    # Earth from the Moon: its size, under the Moon, under the Sun.
    diam, moon_lon, moon_lat, esun_lon, esun_lat = horizons("399", "500@301", "13,14,15")[:5]
    earth = vec(sub_lat, sub_lon)
    sun = vec(sun_lat, sun_lon)
    print(f"Moon under the Sun   {sun_lat:8.3f} {sun_lon:8.3f}")
    print(f"Moon under the Earth {sub_lat:8.3f} {sub_lon:8.3f}")
    print(f"Earth under the Moon {moon_lat:8.3f} {moon_lon:8.3f}")
    print(f"Earth under the Sun  {esun_lat:8.3f} {esun_lon:8.3f}")
    print(f"Earth's radius seen from the Moon {diam / 7200.0:.4f}°")
    phase = math.degrees(math.acos(dot(earth, sun)))
    print(f"Sun–Moon–Earth angle {phase:.2f}° (from the Moon the Earth is {(1 + math.cos(math.radians(180 - phase))) / 2 * 100:.0f} % lit)")

    # Where the spacecraft was: on the great circle from the point under
    # the Earth through Pasteur, as far beyond as puts the Earth 5° above
    # the horizon (which lies `dip` below the level).
    dip = math.degrees(math.acos(MOON_R / (MOON_R + ALTITUDE)))
    elevation = -dip + 5.0
    away = 90.0 - elevation
    pasteur = vec(*PASTEUR)
    pole = norm(cross(earth, pasteur))
    a = math.radians(away)
    craft = add(scale(earth, math.cos(a)), scale(cross(pole, earth), math.sin(a)))
    lat, lon = latlon(craft)
    print(f"horizon dip {dip:.2f}°, so the Earth {elevation:.2f}° from level, {away:.2f}° of arc from under it")
    print(f"spacecraft over {lat:.3f} {lon:.3f}")

    # The orbit through there, inclined 12°, going west: of the two, the
    # one whose path runs nearer the Earth's direction.
    i = math.radians(INCLINATION)
    best = None
    for sign in (1.0, -1.0):
        # Heading (from north through east) at that latitude for a track of
        # that inclination going west: cos(i) = cos(lat) sin(heading).
        s = math.cos(i) / math.cos(math.radians(lat))
        heading = math.degrees(math.asin(min(1.0, s)))
        heading = 180.0 + heading if sign > 0 else 360.0 - heading
        earth_az = azimuth(craft, earth)
        off = (earth_az - heading + 180.0) % 360.0 - 180.0
        print(f"  track heading {heading:7.2f}°, the Earth at azimuth {earth_az:.2f}°, {off:+.2f}° off the track")
        if best is None or abs(off) < abs(best[1]):
            best = (heading, off)
    heading, off = best
    sun_el = math.degrees(math.asin(dot(craft, sun)))
    sun_az = azimuth(craft, sun)
    print(f"chosen heading {heading:.2f}°; the Sun there {sun_el:.2f}° up at azimuth {sun_az:.2f}°")
    speed = 2 * math.pi * (MOON_R + ALTITUDE) / (PERIOD_MIN * 60.0)
    ground = speed * MOON_R / (MOON_R + ALTITUDE)
    # The Earth's axis: the IAU's pole of the Earth (Archinal et al.),
    # turned into the Moon's frame, which the check below vouches for:
    # the point under the Earth from it agrees with Horizons' own.
    jd, earth_icrf = vectors("399")
    to_moon = moon_frame(jd)
    t = (jd - 2451545.0) / 36525.0
    pole_icrf = vec(90.0 - 0.557 * t, -0.641 * t)
    check = latlon(norm(to_moon(earth_icrf)))
    print(f"check: under the Earth by the IAU model {check[0]:.3f} {check[1]:.3f} (Horizons {sub_lat:.3f} {sub_lon:.3f})")
    pole = norm(to_moon(pole_icrf))
    print()
    print("for earthrise.rs, in the frame of the ground under the spacecraft (x east, y up, z south):")
    print(f"  to the Sun        {local(craft, sun)}")
    print(f"  to the Earth      {local(craft, earth)}")
    print(f"  the Earth's pole  {local(craft, pole)}")
    print(f"  the Earth faces the Moon with {moon_lat:.3f}, {moon_lon:.3f}; its radius {diam / 7200.0:.4f}°")
    print(f"  over {lat:.3f} {lon:.3f}, heading {heading:.2f}°")
    print(f"orbital speed {speed * 1000:.1f} m/s, over the ground {ground * 1000:.1f} m/s, {360 / PERIOD_MIN / 60:.5f}°/s")


if __name__ == "__main__":
    main()
