# Where the sites' maps come from

Everything in this directory is cut from public-domain maps made by NASA
or the U.S. Geological Survey, works of the U.S. government, by
`apps/moon/tools/sites.py`. Regenerate it with that script; never edit it
by hand.

| What | Product | Made by | Spacing |
|---|---|---|---|
| Moon heights | LOLA gridded topography, LDEM_512 (`LRO-L-LOLA-4-GDR-V1.0`) | NASA Goddard Space Flight Center, LRO LOLA team | 59 m |
| Moon albedo | Clementine UVVIS 750 nm global mosaic, 118 m, v2 | NASA / USGS Astrogeology (Clementine: BMDO and NASA) | 118 m |
| Mars heights | MOLA Mission Experiment Gridded Data Record, 128 px/deg (`MGS-M-MOLA-5-MEGDR-L3-V1.0`) | NASA Goddard Space Flight Center, MGS MOLA team | 463 m |
| Mars colour | Viking MDIM 2.1 colourised global mosaic, 232 m | NASA / USGS Astrogeology | 232 m |

The heights come from the PDS Geosciences Node
(pds-geosciences.wustl.edu), the mosaics from USGS
(planetarymaps.usgs.gov). Public domain needs no permission, but credit
is asked and gladly given: NASA/GSFC (LOLA, MOLA), NASA/USGS (Clementine,
Viking mosaics).

Deliberately not used, although sharper: SLDEM2015 (LOLA merged with
JAXA's Kaguya stereo heights) and the HRSC–MOLA blend (with ESA's Mars
Express HRSC). Their non-U.S. parts come under JAXA's and ESA's own terms
of use, not the public domain.

The text about each place in `src/sites.rs` and `src/bodies/` is written
for this project from published facts (dates, sizes, findings), not
copied from anywhere.
