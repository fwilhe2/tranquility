# Where the Earth's map comes from

`earth.png` is made by `apps/moon/tools/earth.py` from NASA's **Blue Marble
Next Generation**, December, with topography and bathymetry
(`world.topo.bathy.200412.3x5400x2700.jpg`), by Reto Stöckli, NASA Earth
Observatory (NASA Goddard Space Flight Center), from MODIS data. A work of
NASA, in the public domain; credit as asked: *NASA Earth Observatory, Blue
Marble Next Generation (Reto Stöckli)*.

It is the land and sea without clouds, in December: the season of Apollo 8.
The clouds over it are painted by the app (`src/heavens.rs`) and are not
the weather of any real day; no global picture of the clouds of 24 December
1968 exists to draw them from. They follow December's climate only in
kind: a broken rain belt just south of the equator, clear skies under the
subtropical highs, fronts wound into lows along the storm tracks.

The sea is drawn dark and nearly even, as it looks from space: the map's
sea shows the ocean's depth, pale over the continental shelves, and only a
trace of that is kept.
