//! Real places. Each site is a square of real ground cut out of NASA's
//! global maps by `tools/sites.py`, from public-domain sources only (see
//! `data/sites/SOURCES.md`): heights from the Lunar Orbiter Laser Altimeter
//! for the Moon (59 m between samples) and from the Mars Orbiter Laser
//! Altimeter for Mars (463 m), with the Moon's albedo from Clementine and
//! Mars's colour from Viking. Everything finer than the source is the
//! body's own procedural ground: its craters, ripples, rocks and grain, the
//! part of it a grid at the source's spacing cannot show
//! (`Ground::height_at` leaves that out, so the difference between it at
//! that spacing and at the walker's is exactly the detail). Beyond the
//! square the body's ground goes on, at the square's level, invented.
//!
//! A site carries what there is to know about it, for the card on `I`, and
//! landmarks, named when you look at them.

use std::sync::{Arc, OnceLock};

use tranquility::prelude::*;

use crate::body::Land;

/// A named feature of a site: where (metres east and south of the
/// square's centre), and a sentence.
pub struct Landmark {
    pub name: &'static str,
    pub at: (f64, f64),
    pub about: &'static str,
}

pub struct Site {
    pub id: &'static str,
    pub body: &'static str,
    pub name: &'static str,
    /// Where the walk starts, metres east and south of the square's centre,
    /// and which way you look then (yaw, pitch).
    pub start: (f64, f64),
    pub look: (f32, f32),
    pub note: &'static str,
    pub about: &'static [(&'static str, &'static str)],
    pub landmarks: &'static [Landmark],
    /// Nothing in the sky (the Moon's far side never sees the Earth).
    pub empty_sky: bool,
    heights: &'static [u8],
    colour: &'static [u8],
    map: OnceLock<Arc<Map>>,
}

const INDEX: &str = include_str!("../data/sites/sites.txt");

macro_rules! data {
    ($id:literal) => {
        (include_bytes!(concat!("../data/sites/", $id, ".png")), include_bytes!(concat!("../data/sites/", $id, ".colour.png")))
    };
}

#[allow(clippy::too_many_arguments)]
const fn site(
    id: &'static str,
    body: &'static str,
    name: &'static str,
    start: (f64, f64),
    look: (f32, f32),
    note: &'static str,
    about: &'static [(&'static str, &'static str)],
    landmarks: &'static [Landmark],
    data: (&'static [u8], &'static [u8]),
) -> Site {
    Site { id, body, name, start, look, note, about, landmarks, empty_sky: false, heights: data.0, colour: data.1, map: OnceLock::new() }
}

/// A site the Earth never looks down on.
const fn far_side(mut site: Site) -> Site {
    site.empty_sky = true;
    site
}

pub static SITES: [Site; 11] = [
    site(
        "hadley",
        "Moon",
        "Hadley–Apennine",
        (0.0, 0.0),
        (-0.73, 0.02),
        "Apollo 15's landing site: the mare between the Apennine front and the winding Hadley Rille, where a rover first drove on the Moon.",
        &[
            ("Landing", "David Scott and James Irwin landed Falcon here on 30 July 1971 and stayed three days, with three drives in the first Lunar Roving Vehicle, 28 km in all."),
            ("Hadley Rille", "A channel about 1.5 km wide and 300 m deep that winds across the mare for more than 100 km: most likely a lava channel or a collapsed lava tube."),
            ("The Apennines", "The mountains to the east and south are the rim of the Imbrium basin, thrust up by an impact some 3.9 billion years ago."),
            ("Genesis Rock", "On the slope of Hadley Delta the crew found a piece of anorthosite about 4 billion years old: a fragment of the Moon's first crust."),
            ("Hammer and feather", "Scott dropped a hammer and a falcon feather together; with no air to slow the feather, they reached the ground at the same moment."),
        ],
        &[
            Landmark { name: "Mons Hadley", at: (13_470.0, -14_970.0), about: "Some 4 km above the mare: one of the highest peaks of the Apennine front." },
            Landmark { name: "Hadley Delta", at: (3450.0, 17_190.0), about: "3.5 km above the plain. The crew drove up its flank to Spur crater, where the Genesis Rock lay." },
            Landmark { name: "Hadley Rille", at: (-2250.0, 870.0), about: "About 300 m deep here, its walls strewn with blocks of the lava layers it cuts through." },
        ],
        data!("hadley"),
    ),
    site(
        "tranquility",
        "Moon",
        "Tranquility Base",
        (0.0, 0.0),
        (0.5, -0.08),
        "Where Eagle landed on 20 July 1969: a flat, dark mare plain pocked with small craters, the horizon only two and a half kilometres off.",
        &[
            ("Landing", "Neil Armstrong and Buzz Aldrin set Eagle down here on 20 July 1969, several kilometres past the planned point, after Armstrong flew on over a boulder-strewn crater to find clear ground. They spent about two and a half hours outside."),
            ("Ground", "Mare Tranquillitatis is basalt lava some 3.6 to 3.9 billion years old. The rocks brought back were rich in titanium, and held a mineral new to science, armalcolite, named after the crew."),
            ("The plain", "Across this whole square the ground rises and falls by barely a kilometre: the mare is among the flattest places on the Moon, which is why it was chosen."),
            ("Left behind", "The descent stage, a laser reflector that is still used to measure the distance to the Moon, a seismometer, and the flag, which the ascent engine's blast knocked over."),
        ],
        &[],
        data!("tranquility"),
    ),
    site(
        "taurus",
        "Moon",
        "Taurus–Littrow",
        (0.0, 0.0),
        (2.35, 0.03),
        "The valley where Apollo 17 landed, its dark floor between bright massifs over two kilometres high: the last place people have walked on the Moon.",
        &[
            ("Landing", "Eugene Cernan and Harrison Schmitt landed Challenger here on 11 December 1972 and stayed three days. Schmitt, a geologist, is the only scientist to have walked on the Moon."),
            ("The valley", "A narrow valley between massifs of the Serenitatis basin's rim, floored with dark volcanic ash and with the debris of an avalanche off the South Massif."),
            ("Orange soil", "At Shorty crater Schmitt found orange soil: tiny beads of volcanic glass from fire fountains some 3.6 billion years ago."),
            ("The last steps", "They left on 14 December 1972. Cernan's are the last bootprints on the Moon so far."),
        ],
        &[
            Landmark { name: "North Massif", at: (3150.0, -12_030.0), about: "About 2 km above the valley floor. Boulders that rolled down it were sampled at its foot." },
            Landmark { name: "South Massif", at: (-7950.0, 7770.0), about: "2.3 km above the valley floor. A landslide off its face spread the light mantle across the valley." },
        ],
        data!("taurus"),
    ),
    site(
        "copernicus",
        "Moon",
        "Copernicus crater",
        (0.0, 0.0),
        (2.14, 0.04),
        "On the floor of Copernicus, 93 km across: the central peaks ahead, and all round, tens of kilometres off, the terraced walls.",
        &[
            ("Crater", "Copernicus is 93 km across and nearly 4 km deep, made by an impact some 800 million years ago: young for the Moon, and its bright rays still reach hundreds of kilometres across the mare."),
            ("Peaks", "The central peaks rose as the floor rebounded in the minutes after the impact, lifting rock from deep in the crust."),
            ("Terraces", "The walls slumped inward in great blocks, leaving the stepped terraces round the rim."),
            ("From Earth", "North-west of the Moon's centre as seen from Earth: bright, rayed and easy to find with binoculars."),
        ],
        &[
            Landmark { name: "Central peaks", at: (-5470.0, 3520.0), about: "1.1 km above the floor: rock from the crust's depths, lifted as the floor rebounded." },
            Landmark { name: "The western wall", at: (-46_580.0, -970.0), about: "The crest of the rim, 3.9 km above the floor, terraced on the way down." },
        ],
        data!("copernicus"),
    ),
    far_side(site(
            "vonkarman",
            "Moon",
            "Von Kármán crater",
            (0.0, 0.0),
            (0.5, -0.06),
            "The far side: Chang'e 4's landing site on the dark floor of Von Kármán crater. The Earth is never in this sky.",
            &[
                ("Landing", "China's Chang'e 4 landed here on 3 January 2019, the first spacecraft to land on the far side; its rover Yutu-2 went on driving for years."),
                ("Far side", "This hemisphere never faces the Earth, so a relay satellite, Queqiao, carried the signals home."),
                ("Basin", "Von Kármán is some 180 km across, inside the South Pole–Aitken basin, the largest and oldest known impact basin on the Moon. Its floor was later flooded by mare basalt."),
                ("Quiet", "Shielded from Earth's radio noise, the far side is the quietest place nearby for radio astronomy."),
            ],
            &[],
            data!("vonkarman"),
        )),
    far_side(site(
        "pasteur",
        "Moon",
        "Pasteur",
        (-41_530.0, -1970.0),
        (1.57, 0.02),
        "Just past the Moon's eastern limb, on the floor of Pasteur, a worn basin 224 km across. Apollo 8's crew watched the Earth rise over this ground.",
        &[
            ("Earthrise", "On 24 December 1968, on their fourth orbit, Frank Borman, Jim Lovell and Bill Anders came round from the far side over this region and saw the Earth come up over the horizon. Anders' colour photograph of it is one of the best known ever taken. E brings it back."),
            ("Pasteur", "An old impact basin with a broken, cratered rim, worn down by everything that has hit it since. Its floor is pocked with later craters."),
            ("Names", "In 2018 the IAU named two craters here for the flight: Anders' Earthrise, 40 km across on Pasteur's western floor, in the photograph's foreground, and 8 Homeward, near its left horizon."),
            ("Map", "1,600 km of ground at 1 km spacing from LOLA's 237 m heights: wide enough to be seen from orbit, out to the horizon 600 km off."),
        ],
        &[
            Landmark { name: "Anders' Earthrise", at: (-164_040.0, -7130.0), about: "40 km across, on Pasteur's western floor: the crater in the foreground of the Earthrise photograph." },
        ],
        data!("pasteur"),
    )),
    site(
        "jezero",
        "Mars",
        "Jezero crater",
        (0.0, 0.0),
        (1.57, 0.0),
        "Octavia E. Butler Landing, where Perseverance came down in 2021: the floor of a crater that once held a lake, its delta to the west.",
        &[
            ("Landing", "Perseverance landed here on 18 February 2021 with the Ingenuity helicopter. It drills rock cores and seals them in tubes, for a later mission to bring back to Earth."),
            ("Lake", "Jezero is 45 km across. Some 3.5 billion years ago a river broke in through the western rim and filled it with a lake; the fan of sediment it left, the delta, is among the best places on Mars to look for signs of past life."),
            ("Ingenuity", "Here the helicopter made the first powered flight on another planet, on 19 April 2021, and 71 more before a damaged blade grounded it in January 2024."),
            ("Rocks", "The crater floor turned out to be volcanic, lava and an olivine-rich rock, altered by water; the delta holds mudstones and sandstones."),
        ],
        &[
            Landmark { name: "The delta", at: (-3500.0, -500.0), about: "The front of the delta: layered sediment the river laid down in the lake, tens of metres above the floor." },
            Landmark { name: "Jezero's western rim", at: (-9750.0, 0.0), about: "The river came in through a gap in this rim, the valley Neretva Vallis." },
        ],
        data!("jezero"),
    ),
    site(
        "gale",
        "Mars",
        "Gale crater",
        (-9350.0, -21_370.0),
        (-2.65, 0.06),
        "Bradbury Landing, where Curiosity touched down in 2012: the floor of Gale crater, with Mount Sharp rising five kilometres to the south.",
        &[
            ("Landing", "Curiosity landed here on 6 August 2012, lowered on cables from a rocket-powered sky crane, and has been climbing the lower slopes of Mount Sharp since 2014."),
            ("Gale", "A crater 154 km across, some 3.6 billion years old. Lakes once filled its floor; the rover found mudstone laid down in still water."),
            ("Mount Sharp", "Aeolis Mons, the mountain in the middle, is a stack of layered sediment about 5 km high: the record of Mars drying out, read layer by layer as the rover climbs."),
            ("Habitable", "Within its first year Curiosity found that ancient Gale could have supported microbes: fresh water and the chemical ingredients life uses."),
        ],
        &[Landmark { name: "Mount Sharp", at: (15_500.0, 24_900.0), about: "Aeolis Mons: layer upon layer of sediment, 5.5 km above the crater floor." }],
        data!("gale"),
    ),
    site(
        "olympus",
        "Mars",
        "Olympus Mons",
        (1680.0, 31_880.0),
        (-1.35, -0.1),
        "On the rim of Olympus Mons's summit caldera, nearly 21 km above the Martian datum: the caldera floor three kilometres below, the air a sixth as dense as at the datum.",
        &[
            ("Volcano", "Olympus Mons is a shield volcano some 600 km across and about 22 km high, the tallest known in the solar system; its slopes are so gentle and so broad that, standing on them, you would hardly know it as a mountain."),
            ("Caldera", "The summit holds nested collapse pits up to about 80 km across and 3 km deep, where the magma chamber below emptied and the roof fell in."),
            ("Cliffs", "The volcano's edge drops in cliffs up to 8 km high; beyond them to the north-west spreads a vast apron of landslide debris."),
            ("Air", "The air here is a sixth as dense as at the datum, about a millibar: a helicopter that flies at Jezero cannot lift itself here."),
        ],
        &[Landmark { name: "The caldera floor", at: (24_900.0, 26_700.0), about: "Three kilometres below the rim, where the summit fell in as the magma under it drained away." }],
        data!("olympus"),
    ),
    site(
        "valles",
        "Mars",
        "Valles Marineris",
        (0.0, 0.0),
        (0.22, 0.12),
        "On the floor of Coprates Chasma, in Valles Marineris: the walls rise some nine kilometres to the plateaus north and south.",
        &[
            ("Canyon", "Valles Marineris runs some 4,000 km along the equator, up to 200 km wide and in places more than 7 km deep: on Earth it would cross the United States from coast to coast."),
            ("Origin", "No river cut it. The crust split as the Tharsis volcanoes rose to the west, and the troughs widened as their walls collapsed in landslides."),
            ("Walls", "The walls show layer upon layer of volcanic rock; some troughs hold mounds of sediment laid down later, perhaps in lakes."),
            ("Name", "Named for Mariner 9, which mapped it in 1971 and 1972."),
        ],
        &[
            Landmark { name: "The northern plateau", at: (-15_500.0, -70_100.0), about: "The canyon's rim, nearly 10 km above its floor." },
            Landmark { name: "The southern plateau", at: (-75_500.0, 20_900.0), about: "The southern rim, as high as the northern: the canyon is a rift between them." },
        ],
        data!("valles"),
    ),
    site(
        "endeavour",
        "Mars",
        "Endeavour crater",
        (-9000.0, 1350.0),
        (-1.57, -0.05),
        "On Endeavour's western rim, where Opportunity spent its last seven years: a worn crater 22 km across on the plains of Meridiani.",
        &[
            ("Rover", "Opportunity landed on Meridiani Planum in January 2004 for a 90-day mission and drove for more than 14 years and 45 km, reaching Endeavour in 2011. A planet-wide dust storm ended it in 2018."),
            ("Water", "Its first finds were the 'blueberries', little spheres of haematite, and sulphate rocks laid down in water; on this rim it found veins of gypsum, left by water flowing through cracks."),
            ("Crater", "Endeavour is old and worn: its rim survives as separate hills, the capes Opportunity explored one after another."),
        ],
        &[Landmark { name: "Endeavour's floor", at: (3850.0, 6250.0), about: "Worn and drifted over with sand: the crater is billions of years old." }],
        data!("endeavour"),
    ),
];

/// The sites on a body, in the table's order.
pub fn on(body: &str) -> impl Iterator<Item = &'static Site> {
    SITES.iter().filter(move |s| s.body == body)
}

pub fn by_id(id: &str) -> Option<&'static Site> {
    SITES.iter().find(|s| s.id.eq_ignore_ascii_case(id))
}

/// A site's square: heights (m above the body's reference), its colour
/// (linear, calibrated to albedo), and where it lies.
pub struct Map {
    n: usize,
    spacing: f32,
    /// The source's own sample spacing, m: anything finer is not the map's.
    resolution: f32,
    heights: Vec<f32>,
    cn: usize,
    colour: Vec<Option<[f32; 3]>>,
    /// The mean height along the square's border: the level of the
    /// invented ground beyond it.
    edge: f32,
    /// Latitude and east longitude of the centre, degrees.
    pub centre: (f64, f64),
}

impl Map {
    fn load(site: &Site) -> Self {
        let line = INDEX.lines().find(|l| l.split_whitespace().next() == Some(site.id)).expect("every site is in sites.txt");
        let v: Vec<f64> = line.split_whitespace().skip(1).map(|t| t.parse().expect("a number")).collect();
        let (floor, quantum, spacing, n) = (v[0] as f32, v[1] as f32, v[2] as f32, v[3] as usize);
        let (raw, width, channels) = decode(site.heights);
        assert_eq!(width, n);
        let heights: Vec<f32> = raw.chunks(2 * channels).map(|c| floor + quantum * u16::from_be_bytes([c[0], c[1]]) as f32).collect();
        let (raw, cn, channels) = decode(site.colour);
        let mars = site.body == "Mars";
        let colour = raw
            .chunks(channels)
            .map(|c| {
                if c.iter().all(|v| *v == 0) {
                    // No data: Clementine's gaps.
                    return None;
                }
                Some(if mars { mars_colour([c[0], c[1], c[2]]) } else { moon_albedo(c[0]) })
            })
            .collect();
        let mut edge = 0.0;
        for k in 0..n {
            edge += heights[k] + heights[(n - 1) * n + k] + heights[k * n] + heights[k * n + n - 1];
        }
        Self { n, spacing, resolution: v[6] as f32, heights, cn, colour, edge: edge / (4 * n) as f32, centre: (v[4], v[5]) }
    }

    /// Half the square's width, m.
    pub fn half(&self) -> f32 {
        0.5 * (self.n - 1) as f32 * self.spacing
    }

    /// The map's own height at a point, bicubic between samples.
    pub fn height(&self, x: f64, z: f64) -> f32 {
        let c = (self.n - 1) as f64 / 2.0;
        let (u, v) = (x / self.spacing as f64 + c, z / self.spacing as f64 + c);
        let (i, j) = (u.floor(), v.floor());
        let (fu, fv) = ((u - i) as f32, (v - j) as f32);
        let at = |di: i64, dj: i64| {
            let ii = (i as i64 + di).clamp(0, self.n as i64 - 1) as usize;
            let jj = (j as i64 + dj).clamp(0, self.n as i64 - 1) as usize;
            self.heights[jj * self.n + ii]
        };
        let row = |dj: i64| cubic(at(-1, dj), at(0, dj), at(1, dj), at(2, dj), fu);
        cubic(row(-1), row(0), row(1), row(2), fv)
    }

    /// How much of the map shows at a point: all of it inside, fading to
    /// none over the outer tenth of the square.
    pub fn weight(&self, x: f64, z: f64) -> f32 {
        let half = self.half() as f64;
        let d = x.abs().max(z.abs()) / half;
        let t = ((1.0 - d) / 0.1).clamp(0.0, 1.0) as f32;
        t * t * (3.0 - 2.0 * t)
    }

    /// The map's colour at a point, bilinear; `None` where it has no data.
    pub fn colour(&self, x: f64, z: f64) -> Option<[f32; 3]> {
        let s = 2.0 * self.spacing as f64;
        let c = (self.cn - 1) as f64 / 2.0;
        let (u, v) = (x / s + c, z / s + c);
        if u < 0.0 || v < 0.0 || u >= (self.cn - 1) as f64 || v >= (self.cn - 1) as f64 {
            return None;
        }
        let (i, j) = (u.floor() as usize, v.floor() as usize);
        let (fu, fv) = ((u - i as f64) as f32, (v - j as f64) as f32);
        let mut sum = [0.0f32; 3];
        let mut weight = 0.0;
        for (di, dj, w) in [(0, 0, (1.0 - fu) * (1.0 - fv)), (1, 0, fu * (1.0 - fv)), (0, 1, (1.0 - fu) * fv), (1, 1, fu * fv)] {
            if let Some(c) = self.colour[(j + dj) * self.cn + i + di] {
                for k in 0..3 {
                    sum[k] += w * c[k];
                }
                weight += w;
            }
        }
        (weight > 0.25).then(|| sum.map(|v| v / weight))
    }
}

/// A PNG's samples, its width and the samples to a pixel.
pub fn decode(bytes: &[u8]) -> (Vec<u8>, usize, usize) {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().expect("a site's PNG");
    let mut buf = vec![0; reader.output_buffer_size().expect("a sane size")];
    let info = reader.next_frame(&mut buf).expect("a site's PNG");
    buf.truncate(info.buffer_size());
    (buf, info.width as usize, info.color_type.samples())
}

/// Catmull-Rom between b and c.
fn cubic(a: f32, b: f32, c: f32, d: f32, t: f32) -> f32 {
    b + 0.5 * t * (c - a + t * (2.0 * a - 5.0 * b + 4.0 * c - d + t * (3.0 * (b - c) + d - a)))
}

/// Clementine's 750 nm mosaic counts as reflectance: the mare at
/// Tranquility Base (about 29) is 0.07, Copernicus's fresh walls (about 80)
/// 0.2, as measured. Mature soil reddens a little.
fn moon_albedo(dn: u8) -> [f32; 3] {
    let a = dn as f32 * 0.0025;
    [a * 1.05, a, a * 0.94]
}

/// Viking's colour mosaic, from its display encoding to light, a little
/// brighter for the dusty plains' 0.25 albedo, and its colour pulled most
/// of the way toward the dust's: the mosaic runs blue (more blue than green
/// on average, which no Martian soil is), so it gives the brightness and
/// only a hint of the hue.
fn mars_colour(c: [u8; 3]) -> [f32; 3] {
    let lin = c.map(|v| {
        let v = v as f32 / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    });
    let lum = 0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2];
    let dust = [lum * 1.35, lum * 0.93, lum * 0.66];
    [0, 1, 2].map(|k| 1.3 * (lin[k] + (dust[k] - lin[k]) * 0.75))
}

impl Site {
    pub fn map(&self) -> Arc<Map> {
        self.map.get_or_init(|| Arc::new(Map::load(self))).clone()
    }

    /// Latitude and east longitude of a point of the site, given in metres
    /// east and south of the square's centre, on a body of this radius.
    pub fn lat_lon(&self, radius: f32, at: (f64, f64)) -> (f64, f64) {
        let (lat0, lon0) = self.map().centre;
        let r = radius as f64;
        let lat = lat0 - (at.1 / r).to_degrees();
        let lon = lon0 + (at.0 / (r * lat0.to_radians().cos())).to_degrees();
        (lat, (lon + 180.0).rem_euclid(360.0) - 180.0)
    }

    /// Height of the ground where the walk starts, m above the reference.
    pub fn elevation(&self) -> f32 {
        self.map().height(self.start.0, self.start.1)
    }
}

/// "26.13° N, 3.63° E".
pub fn degrees((lat, lon): (f64, f64)) -> String {
    format!("{:.2}° {}, {:.2}° {}", lat.abs(), if lat >= 0.0 { "N" } else { "S" }, lon.abs(), if lon >= 0.0 { "E" } else { "W" })
}

/// Below this the body's own ground is detail on the map; above it, the
/// invented ground beyond the square keeps only its hills, not its
/// mountains.
const BEYOND: f32 = 1500.0;

/// The body's ground under a site's map.
pub struct SiteLand {
    map: Arc<Map>,
    base: Arc<dyn Land>,
    start: (f64, f64),
}

impl SiteLand {
    pub fn new(site: &Site, base: Arc<dyn Land>) -> Self {
        Self { map: site.map(), base, start: site.start }
    }

    /// The body's ground between two grid steps: what a map of the
    /// coarser cannot show and a mesh of the finer can.
    fn band(&self, x: f64, z: f64, fine: f32, coarse: f32) -> f32 {
        if fine >= coarse {
            return 0.0;
        }
        self.base.height_at(x, z, fine) - self.base.height_at(x, z, coarse)
    }
}

impl Ground for SiteLand {
    fn height(&self, x: f64, z: f64) -> f32 {
        self.height_at(x, z, 0.0)
    }

    fn height_at(&self, x: f64, z: f64, step: f32) -> f32 {
        let w = self.map.weight(x, z);
        let detail = self.map.spacing.max(self.map.resolution);
        let inside = if w > 0.0 { self.map.height(x, z) + self.band(x, z, step, detail) } else { 0.0 };
        if w >= 1.0 {
            return inside;
        }
        let outside = self.map.edge + self.band(x, z, step, BEYOND);
        inside * w + outside * (1.0 - w)
    }

    fn tint(&self, x: f64, z: f64, height: f32, normal: Vec3) -> LinearRgba {
        self.tint_at(x, z, height, normal, 0.0)
    }

    fn tint_at(&self, x: f64, z: f64, height: f32, normal: Vec3, step: f32) -> LinearRgba {
        let own = self.base.tint_at(x, z, height, normal, step);
        let w = self.map.weight(x, z);
        let Some(map) = (w > 0.0).then(|| self.map.colour(x, z)).flatten() else { return own };
        // The map gives the colour at its scale; the body's own tint gives
        // what varies below it (mottling, fresh crater walls, bright
        // slopes), as its brightness against its own tint on level ground
        // far coarser.
        let lum = |c: LinearRgba| 0.2126 * c.red + 0.7152 * c.green + 0.0722 * c.blue;
        let coarse = self.base.tint_at(x, z, height, Vec3::Y, 4000.0);
        let fine = (lum(own) / lum(coarse).max(1e-4)).clamp(0.3, 3.0);
        let mix = |a: f32, b: f32| a + (b - a) * w;
        LinearRgba::new(mix(own.red, map[0] * fine), mix(own.green, map[1] * fine), mix(own.blue, map[2] * fine), own.alpha)
    }

    fn blocked(&self, x: f64, z: f64) -> bool {
        self.base.blocked(x, z)
    }
}

impl Land for SiteLand {
    fn rockiness(&self, x: f64, z: f64) -> f32 {
        self.base.rockiness(x, z)
    }

    fn vents(&self, near: (f64, f64)) -> Vec<(f64, f64)> {
        self.base.vents(near)
    }

    fn landing_site(&self) -> (f64, f64) {
        crate::body::level_site(self, self.start.0, self.start.1, 1.2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every site decodes, and where the walks start agrees with where the
    /// landers came down and how high, as published.
    #[test]
    fn sites_are_where_they_should_be() {
        for site in &SITES {
            let map = site.map();
            assert!(map.half() > 10_000.0, "{}: the square is {} m", site.id, 2.0 * map.half());
        }
        let near = |id: &str, radius: f32, lat: f64, lon: f64, elevation: (f32, f32)| {
            let site = by_id(id).unwrap();
            let (la, lo) = site.lat_lon(radius, site.start);
            assert!((la - lat).abs() < 0.01 && (lo - lon).abs() < 0.01, "{id} starts at {la}, {lo}");
            let e = site.elevation();
            assert!((elevation.0..elevation.1).contains(&e), "{id} at {e} m");
        };
        near("hadley", 1_737_400.0, 26.132, 3.634, (-2100.0, -1800.0));
        near("tranquility", 1_737_400.0, 0.674, 23.473, (-2050.0, -1800.0));
        near("gale", 3_389_500.0, -4.590, 137.442, (-4700.0, -4300.0));
        near("jezero", 3_389_500.0, 18.445, 77.451, (-2700.0, -2450.0));
        // The summit caldera's rim, near 21 km.
        let olympus = by_id("olympus").unwrap();
        assert!((20_000.0..21_500.0).contains(&olympus.elevation()));
    }

    /// The map's height between samples is smooth and passes through them.
    #[test]
    fn heights_interpolate_through_the_samples() {
        let map = by_id("hadley").unwrap().map();
        let s = map.spacing as f64;
        let a = map.height(0.0, 0.0);
        let b = map.height(0.5 * s, 0.0);
        let c = map.height(s, 0.0);
        assert!((b - 0.5 * (a + c)).abs() < 5.0, "{a} {b} {c}");
        assert_eq!(map.weight(0.0, 0.0), 1.0);
        assert_eq!(map.weight(map.half() as f64 + 10.0, 0.0), 0.0);
    }
}
