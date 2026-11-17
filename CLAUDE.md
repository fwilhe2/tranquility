# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A first-person world engine on Bevy 0.20 (Rust), meant to carry what [lunar-walk](https://github.com/fwilhe2/lunar-walk) and historical-walk (both three.js) do, natively and in the browser (wasm + WebGPU). A cargo workspace: `crates/tranquility` is the engine, `apps/{potsdam,moon,island}` are worlds built on it; `moon` is nine bodies (`apps/moon/src/bodies/`, one file each), changed at run time by replacing the `WorldDef` resource. `README.md` maps the two original apps' features onto the engine.

## Running

```sh
make run APP=potsdam                 # native debug (dependencies at opt-level 3)
make release APP=moon
make web APP=moon                    # dist/moon/, needs WebGPU
make shot APP=… OUT=… POSE=x,y,z,yaw,pitch HOUR=…
cargo build -p <app>                 # what the Makefile does
TRANQUILITY_BODY=mars target/debug/moon          # or a first argument
TRANQUILITY_BENCH=10 TRANQUILITY_VSYNC=off target/debug/moon   # frame times
MOON_DRIVE=chase,go TRANQUILITY_SCREENSHOT=out.png target/debug/moon   # at the rover's wheel
MOON_STROLL=1 TRANQUILITY_SHOT_AFTER=40 TRANQUILITY_SCREENSHOT=out.png target/debug/moon   # 40 s into the stroll
```

A cold build of Bevy takes ~25 min on this machine; later builds are incremental. Release (thin LTO, one codegen unit) and wasm builds are slow too.

**Verify visual changes with screenshots**: `TRANQUILITY_SCREENSHOT=out.png` (plus `TRANQUILITY_POSE`, `TRANQUILITY_HOUR`, `TRANQUILITY_SUN`) runs the app, waits until the terrain has settled, saves a frame and quits; read the PNG. The GPU is a small Radeon (Polaris 12); it once hit a gfx ring timeout under load (the kernel reset it, the next run was fine) — watch for `DeviceLost`.

**Measure with `TRANQUILITY_BENCH`, and let the machine cool between runs.** The CPU is a 15 W laptop part that clocks down to a fraction under sustained load (watch `core_throttle_count`): back-to-back runs show frames of 35 ms and 150 ms from the same scene. The benchmark prints the app's own update time beside the frame time; if that is tens of milliseconds, the CPU is throttled or something is rebuilding every frame. The disk has filled up before: `target/debug` is tens of gigabytes.

## Rules

- **The engine never names a world.** Anything a world does differently is a field in `WorldDef` or a method on `Ground`; no branching on a world's name in `crates/tranquility`. City-, moon- or island-specifics live in their app.
- **One height source.** `Ground::height` feeds the chunk meshes (on the compute pool) and the walker; it must be pure and `Send + Sync`. `height_at(step)` and `tint_at(step)` may leave out what a grid of that step cannot show, and must equal `height`/`tint` at the steps a walker sees close up. Vehicles and tracks stand on `vehicle::drawn_height`, the finest mesh rebuilt from the same lattice and triangles: change the triangulation in `terrain::build_chunk` and that together. Things that stand on the ground ask it too. `stand_height` and `blocked` are for what feet meet that is not terrain (decks, rocks, walls) — never a second ground.
- **Positions are `f64` in the world, `f32` near the floating origin.** `Ground` takes world positions; `Transform`s are render space; `Origin::world`/`render` convert. Every root entity is moved when the origin jumps, so keep render-space positions in `Transform`s (or listen for `OriginShifted`). Nothing may repeat: noise comes from `tranquility::noise` (hashed lattice, no permutation table), and the ground shader's lattices have power-of-two denominators so the origin is a whole number of cells.
- **Never change a material every frame.** Bevy prepares every mesh wearing a changed material again. The curve of a round world takes the camera from the view; the sun's shadow maps are drawn flat and fragments look shadows up at the flat position (`rise` in `ground.wesl`, `curve_fragment.wesl`). Things on round ground wear `PropMaterial`.
- **A body is a row.** In `apps/moon`, light, exposure, the walker and the rover's springs follow from `Body`'s numbers (`body.rs`, `suit.rs`); don't tune them per body by eye. A new body is a file in `bodies/` and a line in `bodies::all`.
- **Placement is a pure function of place.** Use `tranquility::terrain::{hash, unit}` (integer hash of cell coordinates and a salt), so a chunk decorated again after streaming back in gets the same things, and place by world cell, not by chunk: the same rock must show on every level. `hash2` floors its inputs; don't feed it fractional offsets expecting variety.
- Coordinates as in historical-walk: x east, z south, y up; yaw 0 looks north, forward is (−sin yaw, −cos yaw); rings flat `[x, z, …]`, outer positive by `geom::poly::area`.
- `Massing` roofs are lower envelopes of planes; walls are broken wherever the envelope bends so wall tops meet roof edges. A new roof shape is a new set of planes in `Massing::planes`, nothing else.
- Facade parameters ride in the mesh: UV 0 = (distance along the outline, height above street level), vertex colour alpha = `facade::Windows::code()`. Change `facade.wesl` and `Windows` together.
- `apps/potsdam/data/potsdam.json` is historical-walk's generated `src/cities/potsdam/city.json`, copied; regenerate it there (`npm run city potsdam`), never hand-edit it.
- **Never change the camera's sky under `AtmosphereEnvironmentMapLight`.** With that light on the camera, an atmosphere or a skybox coming or going at run time (or the light itself being added) hangs the GPU: Mesa's Intel driver resets the context and the app dies on `DeviceLost`. `SkySettings::sky_light` puts it on at spawn and nothing touches it after; worlds that change (the moon's bodies) go without and light the shade with `ambient`. Test world changes with the touring stroll (`MOON_STROLL=tour MOON_STAY=8 TRANQUILITY_BENCH=60`) and check `journalctl -k` for `GPU HANG`.
- Bevy 0.20 specifics: shaders are WESL (`import bevy_pbr::render::…`), embedded with `embedded_asset!` and referenced as `embedded://tranquility/<file>`; events are `Message`s (`add_message`, `MessageWriter`/`MessageReader`); `bevy::prelude::World` exists, hence `WorldDef`.
- Sound is synthesised, never loaded: `tranquility::sound` (a `Voice` is a function from time to a stereo sample, played through Bevy's audio as a custom `Decodable`), `apps/moon/src/sound.rs` the suit. Screenshot and bench runs are silent (`sound::wanted`). Audio needs ALSA's headers on Linux; no gamepad feature (libudev). `MOON_SOUND_WAV=out.wav` renders a minute of a body's sound for measuring.
- The stroll (`apps/moon/src/stroll.rs`, key `0`, `MOON_STROLL=1`) only pushes `Player::steer`, like a pair of hands; it never moves the walker itself. Its view is for watching: no bob, no roll, no fast turns. Keep any motion (gait, swing) on the figure and its shadow, never on the camera.
