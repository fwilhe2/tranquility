# One engine, several worlds: pick one with APP (moon, the default; potsdam,
# island).
#
# Native:  make run APP=moon        (debug, fast to rebuild)
#          make release APP=moon    (optimised binary in target/release/)
#          make ultra APP=moon      (optimised, at the ultra tier: a fast GPU)
# Browser: make web APP=moon        (WebGPU build in dist/moon/)
#          make serve APP=moon      (build, then serve it on http://localhost:8000)
#
# make shot APP=… OUT=…png [POSE=x,y,z,yaw,pitch] [HOUR=…]  saves a frame.
#
# make gallery                    the README's pictures, rendered again
# make stroll                     the moon app touring its bodies, hands off
# make video [VIDEO=stroll.mp4] [SECONDS=600] [SIZE=1920x1080] [QUALITY=high]
#                                 records that, offline, smooth at 60 fps
# make earthrise [CUT=short|long]  Apollo 8's Earthrise as a film, in real time:
#                                 earthrise-short.mp4 (about 2 min) or
#                                 earthrise-long.mp4 (about 5 min), recorded
# make watch-earthrise [CUT=…]    the same film live, with sound
#                                 (QUALITY=ultra here, or for make stroll, too)
#                                 QUALITY=ultra (and SIZE=3840x2160) for the best picture
#                                 on a fast GPU: README, "Ultra"

APP ?= moon
OUT ?= shot.png
VIDEO ?= stroll.mp4
SECONDS ?= 600
SIZE ?= 1920x1080
QUALITY ?= high
CUT ?= short

.PHONY: run release ultra web serve shot stroll video gallery earthrise watch-earthrise check clean

# wasm-bindgen's CLI must match the crate version in Cargo.lock exactly, so a
# matching copy is installed into .tools/ instead of relying on a global one.
WASM_BINDGEN_VERSION := $(shell grep -A1 '^name = "wasm-bindgen"$$' Cargo.lock | sed -n 's/version = "\(.*\)"/\1/p')
WASM_BINDGEN := .tools/bin/wasm-bindgen

$(WASM_BINDGEN): Cargo.lock
	@if [ "$$($(WASM_BINDGEN) --version 2>/dev/null)" != "wasm-bindgen $(WASM_BINDGEN_VERSION)" ]; then \
		cargo install --locked --root .tools wasm-bindgen-cli --version $(WASM_BINDGEN_VERSION); \
	fi
	@touch $@

run:
	cargo run -p $(APP)

release:
	cargo build --release -p $(APP)

ultra:
	cargo build --release -p $(APP)
	target/release/$(APP) --ultra

# The tier for the live targets, only when asked for on the command line.
LIVE_QUALITY = $(if $(filter command line environment,$(origin QUALITY)),--quality=$(QUALITY))

check:
	cargo clippy --workspace
	cargo test --workspace

shot:
	cargo build -p $(APP)
	TRANQUILITY_SCREENSHOT=$(OUT) TRANQUILITY_POSE=$(POSE) TRANQUILITY_HOUR=$(HOUR) target/debug/$(APP)

# The README's gallery: docs/gallery/*.jpg, high quality, no text.
gallery:
	tools/gallery.sh

stroll:
	cargo build --release -p moon
	MOON_STROLL=tour target/release/moon $(LIVE_QUALITY)

earthrise:
	cargo build --release -p moon
	TRANQUILITY_QUALITY=$(QUALITY) TRANQUILITY_SIZE=$(SIZE) TRANQUILITY_RECORD=earthrise-$(CUT).mp4 MOON_EARTHRISE=film,$(CUT) target/release/moon

watch-earthrise:
	cargo build --release -p moon
	MOON_EARTHRISE=film,$(CUT) target/release/moon $(LIVE_QUALITY)

video:
	cargo build --release -p moon
	TRANQUILITY_QUALITY=$(QUALITY) TRANQUILITY_SIZE=$(SIZE) TRANQUILITY_RECORD=$(VIDEO) TRANQUILITY_RECORD_SECONDS=$(SECONDS) MOON_STROLL=tour target/release/moon

web: $(WASM_BINDGEN)
	cargo build --profile wasm-release --target wasm32-unknown-unknown -p $(APP)
	rm -rf dist/$(APP) && mkdir -p dist/$(APP)
	$(WASM_BINDGEN) --target web --no-typescript --out-name app --out-dir dist/$(APP) \
		--remove-name-section --remove-producers-section \
		target/wasm32-unknown-unknown/wasm-release/$(APP).wasm
	sed 's/{{APP}}/$(APP)/g' web/index.html > dist/$(APP)/index.html
	@ls -lh dist/$(APP)/*.wasm

serve: web
	python3 -m http.server -d dist/$(APP) 8000

clean:
	cargo clean
	rm -rf dist .tools
