# tranquility

A small first-person world engine on [Bevy](https://bevy.org) (MIT/Apache-2.0),
shaped after [lunar-walk](https://github.com/fwilhe2/lunar-walk) and
historical-walk. The same Rust code builds native binaries and browser builds
(WebAssembly + WebGPU).

Three worlds show the range:

| App | What it shows | What it exercises |
|---|---|---|
| `potsdam` | Potsdam in 1900, from historical-walk's own generated `city.json` | data-driven ground (terrain grid, river, land use), 2,862 buildings with real roof shapes, a facade shader, walls, bridges, trees, lamps, collision, landmark labels |
| `moon` | a cratered lunar surface | a procedural height function (crater size classes from hashed cells), airless black sky with stars, a fixed low sun, lunar gravity, boulders scattered per chunk |
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

## Controls

Click to capture the mouse, **Esc** to let it go. **WASD** walk, **Shift**
faster, **Space** jump, **F** fly (**Space**/**C** up and down), **[ ]** sun
(time of day, or elevation for a fixed sun), **B** air on/off, **Q** quality
tier, **H** hide text.

## The engine

A world is a [`WorldDef`](crates/tranquility/src/world.rs): a `Ground` (height and
tint functions, plus optional `stand_height` and `blocked` for decks and walls)
and settings for the terrain streamer, sky, water and walker. The engine never
asks which world it is in. Everything else a world has, like buildings, rocks
or trees, is the app's own Bevy systems.

- `terrain`: square chunks streamed around the camera on Bevy's async compute
  pool, at a resolution chosen by distance, with skirts to hide LOD cracks.
  `ChunkMeshed` messages let apps decorate chunks as they arrive (the moon's
  boulders).
- `sky`: Bevy's physically based atmosphere (or none, with a star field), a
  daily sun path by latitude or a fixed sun, cascaded shadows, exposure from
  the sun's height.
- `player`: walk (gravity, jumps, step-down, sliding collision) or fly.
- `geom`: a mesh builder, a raster for painting polygons and lines (land use,
  collision), and `Massing`, which builds a building from its footprint. Every
  roof is the lower envelope of planes over the footprint, as in
  historical-walk's massing.ts. Gabled, hipped, half-hipped, pyramidal,
  mansard, dome and spire roofs are cut exactly where each plane is lowest,
  and walls rise to meet them.
- `ground`: the terrain material, a `StandardMaterial` extension whose WESL
  shader draws detail below the mesh's resolution: cobbles, setts, gravel,
  grass, regolith. A tint function picks one per vertex
  (`Pattern::Cobbles.on(colour)`), and the mesh carries one weight per pattern
  so borders blend cleanly. Detail bends normals and colours only, never
  geometry, so the walker and the drawn ground agree.
- `facade`: a `StandardMaterial` extension whose WESL shader draws window bays,
  sashes, shopfronts, storey bands and a plinth from per-vertex parameters
  written by `Massing`.
- `hud`, `tools`: overlay text (apps can add a line); screenshot and pose
  hooks.

- `quality`: tiers (**Q**; `TRANQUILITY_QUALITY=high|medium|low`) for shadow-map
  size and cascades, SSAO, TAA or FXAA, and bloom. `TRANQUILITY_OFF=ssao,taa,bloom,shadows`
  switches single features off, for measuring.

Rendering is Bevy's: HDR, ACES tone mapping, bloom, TAA and SSAO.

## Performance

On this dev machine (Radeon 540/550-class GPU, 1280×720) every world is
GPU-bound: release and debug builds run at the same frame rate. Over Potsdam
from 260 m up, the frame rate is 20 fps on high, 29 on medium (the default)
and 30 on low. Sun shadows are the largest cost (high goes from 17 to 30 fps
without them); SSAO, TAA and bloom cost a frame or so each. A flat world sets
`TerrainSettings::casts_shadows = false`, since its fine chunks were most of
the triangles in every cascade.

## Toward lunar-walk and historical-walk

What this covers, and what they have that the engine does not (yet):

- **Shared**: one height function as the single source of truth (meshing jobs
  and walker agree), chunk streaming with LOD, a sun path, sky and exposure,
  walk and fly, quality tiers, sub-metre ground detail in the shader.
- **lunar-walk** has more: four nested LOD levels out to 40 km with curvature
  drop, terrain horizon shadows (here the ground casts into the ordinary
  cascades, so crater shadows end at the shadow distance), Hapke photometry,
  micro-craters traced in the ground shader, footprints, the rover,
  per-world tone mapping, eye adaptation, companions in the sky, effects
  (plumes, geysers, dust devils). Each would be engine work here, mostly
  custom materials and render passes.
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
