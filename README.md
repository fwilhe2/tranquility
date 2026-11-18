# tranquility

A small first-person world engine on [Bevy](https://bevy.org) (MIT/Apache-2.0),
shaped after [lunar-walk](https://github.com/fwilhe2/lunar-walk) and
historical-walk. The same Rust code builds native binaries and browser builds
(WebAssembly + WebGPU).

Three worlds show the range:

| App | What it shows | What it exercises |
|---|---|---|
| `potsdam` | Potsdam in 1900, from historical-walk's own generated `city.json` | data-driven ground (terrain grid, river, land use), 2,862 buildings with real roof shapes, a facade shader, walls, bridges, trees, lamps, collision, landmark labels |
| `moon` | nine bodies from Mercury to Pluto, the Moon among them, each unbounded | procedural grounds that never repeat, round worlds with true horizons, real gravity for a suited walker, other atmospheres (Mars, Titan, Pluto), planets in the sky at true size, a drivable rover, a stroll that walks itself and records itself, sound through the suit |
| `island` | an endless island landscape | the smallest complete world: noise heights, sea, Earth's atmosphere and a daily sun |

## Run

Rust 1.97+.

```sh
make run APP=potsdam       # native debug build (APP: potsdam, moon, island)
make release APP=moon      # native optimised build
make serve APP=moon        # browser build on http://localhost:8000
```

`make web` needs the `wasm32-unknown-unknown` target
(`rustup target add wasm32-unknown-unknown`). On first use it installs the
`wasm-bindgen` CLI matching `Cargo.lock` into `.tools/`. The browser build
needs WebGPU, because Bevy's atmosphere uses compute shaders.

To look at a world without sitting at the window:

```sh
make shot APP=potsdam OUT=shot.png POSE=-200,260,500,0,-0.45 HOUR=16.5
```

`POSE` is `x,y,z,yaw,pitch` (and starts in fly mode). `HOUR` sets the clock;
for a fixed sun, `TRANQUILITY_SUN=elevation` does the same. The frame is saved
once the terrain has streamed in, then the app quits.

On Linux, sound needs ALSA's headers (`alsa-lib-devel` on Fedora,
`libasound2-dev` on Debian and Ubuntu).

GitHub Actions (`.github/workflows/ci.yml`) runs clippy and the tests on
every push and pull request, and builds the three browser apps on `main`
(kept as the `web` artifact for two weeks). Compiled dependencies are
cached, so a run takes about five minutes.

## Controls

Click to capture the mouse, **Esc** to let it go. **WASD** walk, **Shift**
faster, **Space** jump, **F** fly (**Space**/**C** up and down), **[ ]** sun
(time of day, or elevation for a fixed sun), **B** air on/off (Mars and Titan), **Q** quality
tier, **M** sound, **H** hide text, **F11** full screen
(`TRANQUILITY_SIZE=1920x1080` sets the window's size, for recording). A
key whose effect would not show (sound, quality, air, the rover out of
reach) says what it did, or why it did nothing, in the middle of the
screen for a few seconds.

In `moon`: **1**–**9** change body (`make run APP=moon` starts on the Moon;
`TRANQUILITY_BODY=mars` or a first argument picks another), **R** climbs
into the rover parked by the landing site and out again (walk up to it
first: from further than five metres R says where it is), **V** changes
between its seat and a camera behind. On Phobos and Enceladus, where nobody
walks, the movement keys fire suit jets while you are off the ground.
**0** starts the stroll (below); `MOON_STROLL=1` starts in it.

## The stroll

Press **0** and the walk walks itself, for watching or recording: a calm,
endless walk on any of the nine bodies.

- **The head faces the planet.** The Earth from the Moon, Mars from
  Phobos, Jupiter from Io and Europa, Saturn from Enceladus, Charon from
  Pluto: whatever hangs in the sky is held in the frame above a low
  horizon, and stays there. Where nothing hangs (Mercury, Mars, Titan) it
  faces the highest skyline in reach, the sun to one side.
- **Nothing moves fast.** No bob, no roll, no swinging round: the head
  eases toward its aim like a heavy camera on a fluid head, and the lens
  is a little longer than the walker's own.
- **The feet go that way too**, at a walk, never a run or a jump, and
  round what is in the way: a few times a second it weighs some thirty
  bearings over the next forty metres for slope, boulders, the rover and
  the sea, and drifts onto the best. On Phobos, where nobody walks, it
  floats two metres up on the suit's jets.
- **It holds no privileges.** It pushes the same stick a player does, so
  grip, slopes, rocks and the streaming ground answer as they would to a
  pair of hands. Any movement key hands the legs back.
- **The sun creeps**, a degree every two minutes, back and forth: on an
  airless world the shadows are the scenery. Your own shadow walks with
  you, legs swinging.
- The text goes; the picture comes up out of black and the body's name
  fades in and out.
- **T** tours the bodies (or start with `MOON_STROLL=tour`): four
  minutes on each, out to black, and on to the next, from Mercury to
  Pluto and round again. `MOON_STAY=seconds` changes the stay.
- In the browser the address does it: `…/#stroll`, or
  `…/#stroll=tour&body=mars&stay=120` (any option, named without its
  `MOON_` or `TRANQUILITY_`). Click once for sound: browsers keep it off
  until then.

## Recording a video

```sh
make video                       # stroll.mp4: ten minutes at 1920×1080
make video VIDEO=reel.mp4 SECONDS=120 SIZE=1280x720
```

builds the release binary and runs it as

```sh
TRANQUILITY_SIZE=1920x1080 TRANQUILITY_RECORD=stroll.mp4 TRANQUILITY_RECORD_SECONDS=600 \
  MOON_STROLL=tour target/release/moon
```

which records the touring stroll offline: the app runs on a clock of
exactly a sixtieth of a second a frame, however long a frame really
takes, at the High tier, and pipes every frame to `ffmpeg` (H.264,
CRF 16); the suit's sound is pulled from its voices a frame's worth at a
time and put beside the picture. The picture is drawn off the screen,
into an image of exactly the size asked for, so a 1080p video comes out
1080p on a laptop whose screen could not show the window. A slow machine makes the same smooth
video, only more slowly: on an Intel UHD 620 two minutes at 1280×720 took
25 minutes in a debug build. Needs `ffmpeg` with libx264 on the path.

One limit does show on a weak GPU: a frame must still finish within the
driver's timeout. Recording at 1080p on the High tier, an Intel UHD 620
went past it as the tour changed into Mars, where the air is computed
(the driver reset the GPU and the recording stopped; at 640×360 it went
through). `make video QUALITY=medium` asks less of each frame.

A recording cut short (the window closed, Ctrl-C, the GPU reset) is
finished with what it has: the frames already drawn come out as the video
asked for, with their sound. `ffmpeg` runs in a process group of its own,
so Ctrl-C reaches only the app, which then closes the video properly.

## Sound

There is no sound from outside on an airless body: nothing carries it.
What you hear comes through the suit, as on Apollo: the backpack's fans
and pump, which never stop; your own breathing in the helmet, quickening
as you work and slowing back over half a minute; each boot striking the
ground, carried up through your bones as a dead thud with the grit of the
soil in it, and the suit's layers working as the leg swings through; the
jets hissing through the backpack frame; the rover's hub motors through
the seat.

Where there is air it carries what it can, from the body's air density
and wind alone: on Mars six millibars of carbon dioxide bring the grit
and the motors faintly and dully, and the wind is a rumble at the edge of
hearing; Titan's air, four times as dense as ours, carries everything,
and its winds are slow. Everything is synthesised as it plays; nothing is
loaded. **M** mutes. If the audio device fails while the app runs
(unplugged, switched, the sound server restarted), the app says so once,
in the log and on the screen; the sound needs a restart to come back.
Screenshot and benchmark runs are silent, and
`MOON_SOUND_WAV=out.wav` renders a minute of a body's sound to a file.

## The moon app

A body is a row of real numbers (gravity, radius, distance from the sun,
albedo, grip) and a ground. Everything else follows from the row:

- **Light.** Sunlight falls with the square of the distance and the sun's
  disc shrinks with it; the camera is exposed for sunlit ground of the
  body's albedo. Noon on Pluto is 1/1560 of Earth's and the stars are out.
- **Walking.** A suited walker of 171 kg: pace from Froude numbers (legs
  are pendulums: 1.4 m/s at a lope on the Moon, 2.1 on Mars), acceleration
  and braking no harder than grip times gravity, a jump from the legs'
  force–velocity curve (0.8 m on the Moon), ballistic flight. Titan's air
  drags.
- **Horizon.** The ground falls away as on a sphere of the body's radius:
  2.4 km to the horizon on the Moon, 200 m on Phobos.
- **Craters** follow the measured equilibrium size–frequency law
  (N(>D) ≈ 0.08 D⁻² up to a saturation diameter set by the surface's
  age), Pike's depth and rim ratios, and a spread of wear from crisp to
  nearly gone. Bodies without craters (Io, Sputnik Planitia) have none.
- **The lander** that brought it stands near the start, its ladder toward
  you: a lunar module whole, as it stood while its crew was out. The
  descent stage in crinkled gold and black foil on four splayed legs,
  each footpad on the ground under it and the contact probes bent where
  the landing folded them; on it the ascent stage, all facets in riveted
  grey panels, with its triangular windows, the hatch over the porch, the
  propellant tanks bulging to either side, the docking tunnel, radar and
  S-band dishes, and a thruster quad at each corner.
- **The rover** is the LRV at its real size, mass and power on a
  rigid-body suspension. It takes six seconds to reach 13 km/h on the
  Moon, because grip is weight.

| Body | Site | What is there |
|---|---|---|
| Mercury | intercrater plains | saturated cratering, a lobate scarp, hollows |
| Moon | a mare's edge | basalt plains with wrinkle ridges under highland massifs; the Earth |
| Mars | layered buttes | dust, dunes of dark sand, ripples, rock fields; a butterscotch sky |
| Phobos | sub-Mars side | grooves and rubble at 0.0006 g; Mars 42° across |
| Io | sulphur plains | frost, lava flows, tilted mountains, no craters; Jupiter |
| Europa | ridged plains | double ridges in seven families, chaos, salt stains; Jupiter |
| Enceladus | a tiger stripe | snow, folds, the stripe's trough and its jets of ice; Saturn with its rings edge-on |
| Titan | a methane shore | cobbles of ice, channels, a sea, orange haze |
| Pluto | Sputnik Planitia | convection cells, pit swarms, ice mountains; Charon |

The grounds are built from what the missions saw, not from their data: no
place here is a real place.

## The engine

A world is a [`WorldDef`](crates/tranquility/src/world.rs): a `Ground` (height and
tint functions, plus optional `stand_height` and `blocked` for decks and walls)
and settings for the terrain streamer, sky, water and walker. The engine never
asks which world it is in. Everything else a world has, like buildings, rocks
or trees, is the app's own Bevy systems.

- `terrain`: square chunks on nested levels streamed around the camera on
  Bevy's async compute pool: each level's chunks are twice the size of the
  level below with the same number of quads, so detail falls off with
  distance and the moon's ground reaches 25 km in some 260 draws. Skirts
  hide the cracks between levels. `ChunkMeshed` messages let apps decorate
  chunks as they arrive (the moon's rocks).
- `origin`, `noise`: the ground can go on for ever. Positions given to a
  `Ground` are `f64`; what is drawn is kept near zero by a floating origin
  that jumps when the camera has gone 3 km; and noise and the ground
  shader's detail come from hashes of integer lattice cells, which neither
  repeat nor lose precision far out.
- `vehicle`: a rigid body on sprung wheels, driven by forces within each
  tyre's grip.
- `sky`: Bevy's physically based atmosphere (or none, with a star field), a
  daily sun path by latitude or a fixed sun, cascaded shadows, exposure from
  the sun's height.
- `player`: walk or fly. Walking is a velocity with forces on it: with the
  default settings a game walk, with a world's grip and gravity that world's.
- `geom`: a mesh builder, a raster for painting polygons and lines (land use,
  collision), and `Massing`, which builds a building from its footprint. Every
  roof is the lower envelope of planes over the footprint, as in
  historical-walk's massing.ts. Gabled, hipped, half-hipped, pyramidal,
  mansard, dome and spire roofs are cut exactly where each plane is lowest,
  and walls rise to meet them.
- `ground`: the terrain material, a `StandardMaterial` extension whose WESL
  shader draws detail below the mesh's resolution: cobbles, setts, gravel,
  grass, and a world's loose soil (craterlets shadowed by their own rims,
  ripples, stones, frost glints, and how the powder answers the light, all
  set by `Soil`). A tint function picks one per vertex
  (`Pattern::Cobbles.on(colour)`), and the mesh carries one weight per pattern
  so borders blend cleanly. Detail bends normals and colours only, never
  geometry, so the walker and the drawn ground agree.
- `facade`: a `StandardMaterial` extension whose WESL shader draws window bays,
  sashes, shopfronts, storey bands and a plinth from per-vertex parameters
  written by `Massing`.
- `horizon`: shadows beyond the sun's shadow maps. The ground round the
  camera is kept as a wide, coarse height map (32 km), and the ground
  shader marches toward the sun through it where the cascades fade out:
  under a low sun crater floors and the far sides of massifs fall into
  shadow at any distance (`TerrainSettings::far_shadows`).
- `sound`: synthesised sound through Bevy's audio. A voice is a function
  from time to a stereo sample, built from noise, filters, glides and
  oscillators; no files, no decoders.
- `hud`, `tools`: overlay text (apps can add a line, and `Hud::say` a
  notice); screenshot and pose hooks, and `TRANQUILITY_KEYS="2:KeyQ,4+3:KeyW"`
  for keys pressed by themselves (seconds after the ground has settled;
  `+` holds a key that long), to see a key's effect in a screenshot. A
  screenshot, benchmark or recording takes no input from the keyboard or
  mouse, so working at the machine meanwhile does not move it.

- `quality`: tiers (**Q**; `TRANQUILITY_QUALITY=high|medium|low`) for shadow-map
  size and cascades, SSAO, TAA or FXAA, and bloom. The first cascade ends
  6–8 m out, so your own shadow and what lies at your feet get texels of
  a few millimetres; edges are filtered (Gaussian) on High and Medium.
  `TRANQUILITY_OFF=ssao,taa,bloom,shadows,softshadow` switches single features off, for
  measuring.

Rendering is Bevy's: HDR, ACES tone mapping, bloom, TAA and SSAO.

## Performance

On this dev machine (a laptop with a Radeon 540/550-class GPU, 1280×720)
every world is GPU-bound once the ground has streamed in:

| | high | medium (default) | low |
|---|---|---|---|
| Moon, on foot | 18 fps | 30 fps | 40 fps |
| Potsdam from 260 m | 20 fps | 29 fps | 30 fps |

Of a medium frame on the Moon, about 14 ms is there whatever is drawn,
9 ms is the ground, 9 ms the sun's shadows, 3 ms the rocks and 2.5 ms TAA
and bloom. `TRANQUILITY_BENCH=10` measures this (frame time percentiles,
the app's own share, how long the ground took to appear);
`TRANQUILITY_VSYNC=off` unties it from the display. The app's own update is
under 2 ms.

The far shadows (`horizon`) cost nothing measurable from a walker's eye,
where the far ground is a thin band at the horizon, and about a fifth of
the frame from 600 m up, where it is most of the picture (measured on the
laptop's Intel UHD 620: 163 ms a frame with them, 136 without). The stroll, the sound and
the figure under the eye cost the app's update well under a millisecond.

The ground takes about ten seconds to stream in on two cores. The engine
and the moon app are compiled optimised in dev builds for that reason: at
`opt-level = 1` it took a minute, long enough for a laptop CPU to throttle.

## Toward lunar-walk and historical-walk

What this covers, and what they have that the engine does not (yet):

- **Shared**: one height function as the single source of truth (meshing jobs
  and walker agree), chunk streaming with LOD, a sun path, sky and exposure,
  walk and fly, quality tiers, sub-metre ground detail in the shader.
- **lunar-walk** has more: twenty-one bodies to these nine, full Hapke
  photometry, micro-craters traced in the ground shader, solid rocks you
  can stand on, landers, eye adaptation, night by planetshine, effects
  (plumes, geysers, dust devils), a demo that tours the bodies and drives
  the rover. Shared now: terrain shadows at any range, footprints, sound
  through the suit, a walk that walks itself. Here only: nested levels
  with curvature, the rover with tracks and dust, companions in the sky,
  other atmospheres, the walker's shadow.
- **historical-walk** has more: ground textures laid along street frames
  (patterns here are world-aligned, and street edges are as soft as the 1 m
  vertex colours), the traced kerb, sky-view occlusion on walls, facade relief
  (cornices, pilasters, brick), clouds, water reflections and ripples, sound,
  the historical map overlay, the guided tour.

## Licences

Code: MIT OR Apache-2.0. `crates/tranquility/assets/fonts`: Liberation Sans, SIL
Open Font License 1.1. `apps/potsdam/data/potsdam.json` is generated by
historical-walk from OpenStreetMap (ODbL), OpenHistoricalMap (CC0) and the
Messtischblatt of 1903 (dl-de/by-2-0); the app shows its credits.
