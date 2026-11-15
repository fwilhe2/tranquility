# One engine, several worlds: pick one with APP (island, moon, potsdam).
#
# Native:  make run APP=moon        (debug, fast to rebuild)
#          make release APP=moon    (optimised binary in target/release/)
# Browser: make web APP=moon        (WebGPU build in dist/moon/)
#          make serve APP=moon      (build, then serve it on http://localhost:8000)
#
# make shot APP=… OUT=…png [POSE=x,y,z,yaw,pitch] [HOUR=…]  saves a frame.

APP ?= potsdam
OUT ?= shot.png

.PHONY: run release web serve shot check clean

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

check:
	cargo clippy --workspace
	cargo test --workspace

shot:
	cargo build -p $(APP)
	TRANQUILITY_SCREENSHOT=$(OUT) TRANQUILITY_POSE=$(POSE) TRANQUILITY_HOUR=$(HOUR) target/debug/$(APP)

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
