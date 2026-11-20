#!/usr/bin/env bash
# Renders the README's gallery: one frame per shot below, high quality,
# 1920×1080, no text on screen, saved as docs/gallery/<name>.jpg, and the
# README's gallery section rewritten from the same list.
#
#   tools/gallery.sh                 every shot
#   tools/gallery.sh gale titan      just these
#   COOL=0 tools/gallery.sh          no pause between runs
#   BUILD=release tools/gallery.sh   from the optimised build
#
# Each run waits for the ground to stream in, so a shot takes a minute or
# two; the pause between runs lets a laptop CPU cool down, so the next
# frame's timing (and with it the stroll, the flight, the descent) is not
# thrown by a throttled clock. A frame that does not come out (a hidden
# window loses its surface) is tried once more.
set -euo pipefail
cd "$(dirname "$0")/.."

BUILD=${BUILD:-debug}
COOL=${COOL:-20}
OUT=docs/gallery
WIDTH=${WIDTH:-1600}

# name | caption | settings (environment for the app)
SHOTS=(
  "moon-hadley|The Moon: Hadley–Apennine, where Apollo 15 landed, with Mons Hadley to the north-east (LOLA heights, Clementine albedo)|TRANQUILITY_SITE=hadley"
  "moon-descent|Flying the lunar module's last minute down to Hadley, a few metres up in P65|TRANQUILITY_SITE=hadley MOON_DESCENT=chase,auto,lowgate TRANQUILITY_SHOT_AFTER=30"
  "moon-copernicus|On the floor of Copernicus, its central peaks ahead|TRANQUILITY_SITE=copernicus TRANQUILITY_SUN=28,100"
  "mars-gale|Mars: Gale crater from Bradbury Landing, Mount Sharp 50 km off (MOLA heights, Viking colour)|TRANQUILITY_SITE=gale"
  "mars-olympus|The rim of Olympus Mons's summit caldera, 21 km up, the sky dark overhead|TRANQUILITY_SITE=olympus"
  "mars-ingenuity|Ingenuity climbing away at Jezero, flown through the rotor physics|TRANQUILITY_SITE=jezero MOON_FLY=behind,up,side TRANQUILITY_SHOT_AFTER=17"
  "earthrise|Earthrise, 24 December 1968: the Earth over the Moon's far side as Apollo 8's 250 mm lens saw it at 75:48:39, turned as the photograph was printed (geometry from JPL Horizons, LOLA heights, Blue Marble Earth)|MOON_EARTHRISE=75:48:39,lens,printed,still"
  "earthrise-window|Apollo 8, 75:47:32: the Earth coming up past the limb in Anders' window as Borman rolls the spacecraft, the horizon upright as the crew lay|MOON_EARTHRISE=75:47:32,still"
  "io-jupiter|Io's sulphur plains under Jupiter, twenty degrees across|TRANQUILITY_BODY=io TRANQUILITY_LOOK=0.43,0.3"
  "enceladus|Enceladus: a tiger stripe venting, Saturn's rings edge-on|TRANQUILITY_BODY=enceladus"
  "titan|Titan: the shore of a methane sea under orange haze|TRANQUILITY_BODY=titan"
  "pluto|Pluto: Sputnik Planitia's nitrogen ice, Charon in the sky|TRANQUILITY_BODY=pluto"
)

wanted=("$@")
want() {
  [ ${#wanted[@]} -eq 0 ] && return 0
  local n
  for n in "${wanted[@]}"; do [ "$n" = "$1" ] && return 0; done
  return 1
}

if [ "$BUILD" = release ]; then
  cargo build --release -p moon
else
  cargo build -p moon
fi
BIN=target/$BUILD/moon
mkdir -p "$OUT"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

first=1
for shot in "${SHOTS[@]}"; do
  IFS='|' read -r name caption settings <<<"$shot"
  want "$name" || continue
  [ $first -eq 1 ] || sleep "$COOL"
  first=0
  for attempt in 1 2; do
    echo "== $name (attempt $attempt)"
    rm -f "$tmp/$name.png"
    if [ $attempt -eq 1 ]; then
      env TRANQUILITY_QUALITY=high TRANQUILITY_SIZE=1920x1080 TRANQUILITY_CLEAN=1 \
        TRANQUILITY_SHOT_AFTER=4 $settings TRANQUILITY_SCREENSHOT="$tmp/$name.png" \
        timeout 600 "$BIN" >"$tmp/$name.log" 2>&1 || true
    else
      # Drawn off the screen instead: a second of video, its last frame.
      # Some views lose the window's surface just when the shot is due.
      env TRANQUILITY_QUALITY=high TRANQUILITY_SIZE=1920x1080 TRANQUILITY_CLEAN=1 \
        $settings TRANQUILITY_RECORD="$tmp/$name.mp4" TRANQUILITY_RECORD_SECONDS=1 \
        timeout 900 "$BIN" >"$tmp/$name.log" 2>&1 || true
      [ -s "$tmp/$name.mp4" ] && ffmpeg -loglevel error -y -sseof -0.1 -i "$tmp/$name.mp4" -frames:v 1 "$tmp/$name.png" || true
    fi
    if [ -s "$tmp/$name.png" ]; then
      magick "$tmp/$name.png" -resize "${WIDTH}x" -quality 86 "$OUT/$name.jpg"
      echo "   $OUT/$name.jpg"
      break
    fi
    grep -iE "panic|error" "$tmp/$name.log" | tail -3 || true
    [ $attempt -eq 2 ] && echo "   no frame for $name" >&2
    sleep "$COOL"
  done
done

# The README's gallery, between its markers, from every shot that has a
# picture.
section="$tmp/gallery.md"
{
  echo "<!-- gallery: written by tools/gallery.sh -->"
  for shot in "${SHOTS[@]}"; do
    IFS='|' read -r name caption _ <<<"$shot"
    [ -f "$OUT/$name.jpg" ] || continue
    echo
    echo "![$caption]($OUT/$name.jpg)"
    echo "*$caption*"
  done
  echo
  echo "<!-- /gallery -->"
} >"$section"
if grep -q "<!-- gallery" README.md; then
  python3 - "$section" <<'EOF'
import re, sys
section = open(sys.argv[1]).read().rstrip("\n")
text = open("README.md").read()
text = re.sub(r"<!-- gallery.*?<!-- /gallery -->", lambda _: section, text, flags=re.S)
open("README.md", "w").write(text)
EOF
  echo "README.md: gallery rewritten"
else
  echo "README.md has no gallery markers; add <!-- gallery --> <!-- /gallery --> where it should go"
fi
