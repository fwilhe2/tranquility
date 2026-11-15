# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A first-person world engine on Bevy 0.20 (Rust), meant to carry what [lunar-walk](https://github.com/fwilhe2/lunar-walk) and historical-walk (both three.js) do, natively and in the browser (wasm + WebGPU). A cargo workspace: `crates/tranquility` is the engine, `apps/{potsdam,moon,island}` are worlds built on it. `README.md` maps the two original apps' features onto the engine.

## Running

```sh
make run APP=potsdam                 # native debug (dependencies at opt-level 3)
make release APP=moon
make web APP=moon                    # dist/moon/, needs WebGPU
make shot APP=… OUT=… POSE=x,y,z,yaw,pitch HOUR=…
cargo build -p <app>                 # what the Makefile does
```

A cold build of Bevy takes ~25 min on this machine; later builds are incremental. Release (thin LTO, one codegen unit) and wasm builds are slow too.

**Verify visual changes with screenshots**: `TRANQUILITY_SCREENSHOT=out.png` (plus `TRANQUILITY_POSE`, `TRANQUILITY_HOUR`, `TRANQUILITY_SUN`) runs the app, waits until the terrain has settled, saves a frame and quits; read the PNG. The GPU is a small Radeon (Polaris 12); it once hit a gfx ring timeout under load (the kernel reset it, the next run was fine) — watch for `DeviceLost`.

## Rules

- **The engine never names a world.** Anything a world does differently is a field in `WorldDef` or a method on `Ground`; no branching on a world's name in `crates/tranquility`. City-, moon- or island-specifics live in their app.
- **One height source.** `Ground::height` feeds the chunk meshes (on the compute pool) and the walker; it must be pure and `Send + Sync`. Things that stand on the ground ask it too. `stand_height` and `blocked` are for what feet meet that is not terrain (decks, rocks, walls) — never a second ground.
- **Placement is a pure function of place.** Use `tranquility::terrain::{hash, unit}` (integer hash of cell coordinates and a salt), so a chunk decorated again after streaming back in gets the same things. `hash2` floors its inputs; don't feed it fractional offsets expecting variety.
- Coordinates as in historical-walk: x east, z south, y up; yaw 0 looks north, forward is (−sin yaw, −cos yaw); rings flat `[x, z, …]`, outer positive by `geom::poly::area`.
- `Massing` roofs are lower envelopes of planes; walls are broken wherever the envelope bends so wall tops meet roof edges. A new roof shape is a new set of planes in `Massing::planes`, nothing else.
- Facade parameters ride in the mesh: UV 0 = (distance along the outline, height above street level), vertex colour alpha = `facade::Windows::code()`. Change `facade.wesl` and `Windows` together.
- `apps/potsdam/data/potsdam.json` is historical-walk's generated `src/cities/potsdam/city.json`, copied; regenerate it there (`npm run city potsdam`), never hand-edit it.
- Bevy 0.20 specifics: shaders are WESL (`import bevy_pbr::render::…`), embedded with `embedded_asset!` and referenced as `embedded://tranquility/<file>`; events are `Message`s (`add_message`, `MessageWriter`/`MessageReader`); `bevy::prelude::World` exists, hence `WorldDef`.
- No audio or gamepad features (they need alsa/libudev, not installed); add them deliberately.
