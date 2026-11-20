# Where the Earthrise comes from

The scene (`src/earthrise.rs`, key **E**) puts together public records of
Apollo 8's fourth lunar orbit, 24 December 1968. Its numbers come from:

| What | Source |
|---|---|
| Where the Sun and the Earth stood, seen from the Moon; the point on the Earth facing the Moon, the Earth's size | JPL Horizons, Solar System Dynamics Group, NASA Jet Propulsion Laboratory (ssd.jpl.nasa.gov), DE441 ephemeris, queried by `tools/earthrise.py` |
| The Moon's orientation, and the Earth's pole | The IAU Working Group on Cartographic Coordinates and Rotational Elements, report for 2009 (Archinal et al., *Celestial Mechanics and Dynamical Astronomy* 109, 2011): rotation models, as formulas |
| The times of the photographs, the onboard words, the roll, which window saw what | The Apollo Flight Journal, *Apollo 8, Day 4: Lunar Orbit 4* (apollojournals.org/afj/ap08fj/), edited by David Woods and Frank O'Brien, and the onboard voice transcript it gives (NASA) |
| The exact timing (75:48:39 for AS08-14-2383), the roll as the cause of the Earth coming into view, the attitude nose down | NASA Scientific Visualization Studio, *Earthrise* (2013), by Ernie Wright with Andrew Chaikin (svs.gsfc.nasa.gov/4129) |
| The orbit (about 112 km, 12° inclined, retrograde, 128.7 minutes), the spacecraft near 110° E with the Earth some 5° above the horizon, the 250 mm lens and 1/250 s at f/11 | NASA's mission reports and photo caption for AS08-14-2383; collected in Wikipedia's articles *Earthrise* and *Apollo 8* |
| Craters named for the flight (Anders' Earthrise, 8 Homeward) | IAU Gazetteer of Planetary Nomenclature (USGS), named 2018 |
| The ground | `data/sites/SOURCES.md` (LOLA heights, Clementine albedo) |
| The Earth's face | `data/earth/SOURCES.md` (Blue Marble Next Generation) |

The crew's words are quoted from NASA's transcript, a work of the U.S.
government; the Flight Journal's own commentary is not copied. Everything
else written in the app about the flight is in our own words, from the
facts above.

What no source gives, the scene makes up and says so: the rate of Borman's
roll (a steady 1.1°/s, timed so the Earth comes into window 5 and then the
hatch window when the tape has the crew see it there), the shape of the
windows, and the Earth's clouds, which are not that day's weather.
