# Earthrise: open items

Where the Apollo 8 Earthrise work (`src/earthrise.rs`, key **E**) stands,
and what is left. Done so far: exact sphere curve and sphere lighting,
terrain levels by altitude, the sky turning with travel, frustum
refinement for narrow views, the Pasteur site (LOLA/Clementine), the real
Earth (Blue Marble), the scene with its clock, roll, views, words and
shutter, credits in `data/earthrise/SOURCES.md`.

## Open

- [ ] **Gallery pictures not rendered yet.** `tools/gallery.sh earthrise
      earthrise-window` has the two shots and a recording fallback; run it
      (a few minutes each), then commit `docs/gallery/earthrise*.jpg` and the
      README's rewritten gallery.
- [ ] **Screenshots fail in the lens view.** With `TRANQUILITY_SCREENSHOT`
      in the 250 mm view Bevy logs "Unknown window for screenshot,
      skipping" every time (window and hatch views work; frame times are
      fine, 35–45 fps). Seen only since frustum refinement
      (`terrain::refine`) went in. Suspect: continuous chunk uploads as the
      plan follows the turning view, making the swapchain acquire miss that
      frame. Check for dropped frames on screen, too. Workaround: the
      recorder (`TRANQUILITY_RECORD`, last frame through ffmpeg).
- [x] **README.** A section on the Earthrise under "The moon app", and `E`
      under "Controls". Nothing in the README mentions the scene yet.
- [ ] **Verify the new clock start** (it waits for the ground to settle,
      and `still` holds it). Pressing E while walking may start the clock
      a frame early, because the old world's chunks still count as
      settled.
- [ ] **Exposure.** The photograph was 1/250 s at f/11 on SO-368 (ASA 64),
      about EV100 15.5. The scene uses the Moon's usual exposure, and the
      ground looks brighter and flatter than in the photograph. Consider an
      `exposure_bias` for the scene, measured against the original frame.
- [ ] **Ground near the limb** still reads as streaks more than craters,
      even after refinement. Try a lower `STEP_OVER_DISTANCE` for the lens,
      or a finer map than 1 km at 300–600 km ahead (LDEM_128 is 237 m).
- [ ] **The roll is a guess**: a steady 1.1°/s, with window 5 facing the
      track at 76°. If the SVS reconstruction's roll timing can be found
      (svs.gsfc.nasa.gov/4129, data downloads), use it and credit it.
- [ ] **Window geometry** is simplified: every window 57° off the nose, the
      hatch 70° round from window 5, one rounded frame for all of them. The
      real windows' shapes and orientations would make the view more
      faithful.
- [x] **Sound.** `src/cabin.rs`: fans, 400 Hz, the glycol pump, the
      service module's jets (bursts as the roll starts and stops, deadband
      fdocs/gallery/earthrise.jpgirings between), the Hasselblad's shutter and winder, a magazine.
      The suit is silent aboard.
- [x] **Film.** `MOON_EARTHRISE=film[,short]`, `make earthrise CUT=…`:
      directed views, subtitles, title and closing card, black and white
      through the lens until the colour magazine.
- [ ] **Magazine time** (75:48:12) is a guess between the tape's lines;
      the roll's ease-out (5 s, like its start) too.
- [ ] **Crew voices** are subtitles only. Synthesised speech would cheapen
      it; the tape itself is public domain (NASA) but the rule is that
      sound is synthesised, never loaded: leave as is unless that changes.
- [ ] **The spacecraft itself.** There is no command module model. A
      simple one would help the free view, and could show in the windows'
      edges.
- [ ] **Clouds** are procedural, not 24 December 1968's weather (no global
      record exists); they now follow December's climate in kind (ITCZ,
      subtropical clear skies, lows in the storm tracks). Could be painted
      loosely after the photograph.
- [ ] **Browser build** not tried with the scene: `make web APP=moon` and
      `#earthrise=75:48:39,lens` (options read the address hash). The
      embedded data has grown: Pasteur 4.4 MB, Earth 2.6 MB.
- [ ] **Ultra** (`QUALITY=ultra`) is untried: record the films with it on
      a GPU that meets the README's table, and check the motion blur and
      the finer ground in the lens view.
- [ ] **Benchmark** the touring stroll and the other bodies after the
      engine changes (span, refine, sky turn). On the ground they should
      be unchanged; Hadley's look was checked against the gallery.
- [ ] Rule check: `CLAUDE.md` could mention `terrain::refine`/`span` and
      `sky::turn` (directions in the sky are given at the world's zero).
