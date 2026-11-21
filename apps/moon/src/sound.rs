//! What reaches the ear in a pressure suit, generated: after lunar-walk's
//! `sound.ts`.
//!
//! In vacuum nothing outside reaches you at all. What you hear comes
//! through the suit: the fans and the pump of the backpack, which never
//! stop; your own breathing in the helmet, quickening as you work and
//! slowing back over half a minute; your boots striking the ground,
//! carried up through your bones as a dead thud with the grit of the soil
//! in it, and the suit's fabric working with each step; the jets hissing
//! through the backpack frame; the rover's hub motors through the seat.
//! Apollo crews heard their rover only that way.
//!
//! Where there is air it carries sound as well as it can. Mars's six
//! millibars pass about a tenth of the pressure Earth's air would, and
//! carbon dioxide soaks up the high frequencies within metres: the grit
//! and the motors come through faintly and dully, and the wind is a rumble
//! at the edge of hearing. Titan's air is four times as dense as ours and
//! carries everything; its winds are slow. Both follow from the body's
//! `air_density` and `wind`: nothing here is set per body.
//!
//! A helicopter is heard only through the air: the buzz of its blades
//! passing (2537 rpm and two blades make Ingenuity's 85 Hz, which
//! Perseverance's microphones caught from 80 m away) and the hiss off
//! their tips, louder the harder the rotors work and the nearer you are,
//! and on Mars dull and faint, as that recording is.
//!
//! Everything is synthesised from noise and oscillators
//! (`tranquility::sound`). `MOON_SOUND_WAV=out.wav` renders a minute of a
//! walk to a file instead of running the app, for measuring what cannot be
//! listened to from a terminal.

use std::sync::{Arc, Mutex};

use tranquility::{
    prelude::*,
    sound::{Biquad, Glide, Muted, Noise, RATE, Synth, Voice, Wave, play, soften, wanted},
    vehicle::Vehicle,
};

use crate::{
    Current,
    body::Body,
    gait::{Footfall, Striding},
    rotorcraft::Helicopter,
    rover::{Driving, Rover},
};
use tranquility::rotor::Rotorcraft;

/// Everything is this much louder than lunar-walk's levels, which leaned
/// on a compressor.
const LEVEL: f32 = 2.6;
/// The voice reads what the app last said this often, in samples.
const BLOCK: u32 = 256;

/// What the air does to sound, from its density and the wind.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Medium {
    /// Share of a sound made outside that arrives, 0–1, and the frequency
    /// above which it does not, Hz.
    pub carries: f32,
    pub dull: f32,
    /// Level of the wind on the helmet, and how low it rumbles, Hz.
    pub wind: f32,
    pub rumble: f32,
}

impl Medium {
    pub fn of(body: &Body) -> Self {
        // Sound pressure goes with the root of the density (the same
        // source working on a thinner gas).
        let carries = (body.air_density / 1.2).sqrt().min(1.0);
        // Wind noise goes with dynamic pressure, ½ρv²: a breeze of 0.4 m/s
        // on Titan and one of 7 m/s on Mars both press half a pascal.
        let q = 0.5 * body.air_density * body.wind * body.wind;
        Self { carries, dull: 1200.0 + 7800.0 * carries, wind: (0.3 * q.sqrt()).min(0.35), rumble: 380.0 - 240.0 * carries }
    }
}

/// What the app tells the voice, read once per block.
#[derive(Default)]
struct Said {
    medium: Medium,
    /// 0–1: how hard the walker is working.
    work: f32,
    /// Jets firing, 0–1 and over.
    jets: f32,
    /// The rover you sit in: speed as a share of 13 km/h, and throttle.
    rover: Option<(f32, f32)>,
    /// A rotorcraft turning: blade-passage frequency, Hz; level at the
    /// ear before the air takes its share; and −1 left to 1 right.
    rotor: Option<(f32, f32, f32)>,
    /// Boots come down since the last block: strength, and −1 left to 1.
    steps: Vec<(f32, f32)>,
    /// 0–1: how much of it to hear (the picture's fades take the sound
    /// with them).
    level: f32,
}

#[derive(Resource, Clone)]
struct Line(Arc<Mutex<Said>>);

/// One boot on the ground, sounding.
struct Step {
    age: u32,
    strength: f32,
    pan: f32,
    thud: Wave,
    grit: Biquad,
    rustle: Biquad,
}

/// A breath: in, then out, at a rate and depth set when it began.
struct Breath {
    at: f32,
    period: f32,
    depth: f32,
    band: Biquad,
    level: Glide,
    out: bool,
}

/// The suit, and the air outside it.
pub struct Suit {
    line: Arc<Mutex<Said>>,
    noise: Noise,
    clock: u32,
    said: Medium,
    /// Through the suit: the helmet rings a little round 900 Hz and passes
    /// almost nothing high. Left and right.
    ring: [Biquad; 2],
    helmet: [Biquad; 2],
    /// Through the air outside.
    outside: [Biquad; 2],
    carries: Glide,
    fan: [Biquad; 2],
    flow: [Biquad; 2],
    hum: [Wave; 3],
    drift: Wave,
    breath: Breath,
    exertion: f32,
    work: f32,
    steps: Vec<Step>,
    jets: Glide,
    jet: [Biquad; 2],
    motor: Wave,
    gear: Wave,
    motor_band: Biquad,
    motor_pitch: Glide,
    motor_level: Glide,
    gear_level: Glide,
    wind: [Biquad; 2],
    wind_level: Glide,
    gust_in: f32,
    rotor: Wave,
    thump: Wave,
    rotor_band: Biquad,
    tips: Biquad,
    rotor_pitch: Glide,
    rotor_level: Glide,
    rotor_pan: Glide,
    tips_at: f32,
    /// Fades in at the start.
    master: Glide,
}

impl Suit {
    fn new(line: Arc<Mutex<Said>>) -> Self {
        let pair = |f: Biquad| [f.clone(), f];
        let mut master = Glide::new(0.0, 1.0);
        master.send(1.0, 1.5);
        Self {
            line,
            noise: Noise::new(1969),
            clock: 0,
            said: Medium::default(),
            ring: pair(Biquad::peaking(900.0, 1.2, 4.0)),
            helmet: pair(Biquad::lowpass(3200.0, 0.707)),
            outside: pair(Biquad::lowpass(20_000.0, 0.707)),
            carries: Glide::new(0.0, 0.3),
            fan: pair(Biquad::bandpass(700.0, 0.8)),
            flow: pair(Biquad::lowpass(1800.0, 0.6)),
            hum: [Wave::default(); 3],
            drift: Wave::default(),
            breath: Breath { at: 0.0, period: 5.0, depth: 0.0, band: Biquad::bandpass(1300.0, 1.1), level: Glide::new(0.0, 0.1), out: false },
            exertion: 0.15,
            work: 0.0,
            steps: Vec::new(),
            jets: Glide::new(0.0, 0.03),
            jet: [Biquad::highpass(900.0, 0.707), Biquad::lowpass(5200.0, 0.707)],
            motor: Wave::default(),
            gear: Wave::default(),
            motor_band: Biquad::lowpass(1400.0, 0.8),
            motor_pitch: Glide::new(0.0, 0.08),
            motor_level: Glide::new(0.0, 0.1),
            gear_level: Glide::new(0.0, 0.1),
            wind: pair(Biquad::lowpass(380.0, 0.707)),
            wind_level: Glide::new(0.0, 1.5),
            gust_in: 0.0,
            rotor: Wave::default(),
            thump: Wave::default(),
            rotor_band: Biquad::lowpass(900.0, 0.8),
            tips: Biquad::bandpass(600.0, 0.9),
            rotor_pitch: Glide::new(0.0, 0.1),
            rotor_level: Glide::new(0.0, 0.1),
            rotor_pan: Glide::new(0.0, 0.1),
            tips_at: 600.0,
            master,
        }
    }

    /// Takes in what the app has said since the last block.
    fn listen(&mut self) {
        let dt = BLOCK as f32 / RATE;
        // Never wait for the app: if it holds the line, next block will do.
        if let Ok(mut said) = self.line.try_lock() {
            if said.medium != self.said {
                self.said = said.medium;
                self.carries.send(said.medium.carries, 0.3);
                for f in &mut self.outside {
                    f.retune(Biquad::lowpass(said.medium.dull, 0.707));
                }
                for f in &mut self.wind {
                    f.retune(Biquad::lowpass(said.medium.rumble, 0.707));
                }
            }
            self.work = said.work;
            if (self.master.to - said.level).abs() > 1e-3 {
                self.master.send(said.level, 0.6);
            }
            self.jets.send(0.06 * said.jets.min(3.0), 0.03);
            let (speed, drive, on) = said.rover.map_or((0.0, 0.0, 0.0), |(v, d)| (v.min(1.4), d, 1.0));
            self.motor_pitch.send(speed, 0.08);
            self.motor_level.send(on * (0.012 + 0.05 * drive + 0.02 * speed.min(1.0)), 0.1);
            self.gear_level.send(on * 0.006 * speed.min(1.0), 0.1);
            match said.rotor {
                Some((bpf, level, pan)) => {
                    self.rotor_pitch.send(bpf, 0.1);
                    self.rotor_level.send(level, 0.1);
                    self.rotor_pan.send(pan, 0.1);
                    // The tips' turbulence sits a few octaves over the
                    // buzz; retuned only when the rotor speed has moved.
                    let centre = (6.0 * bpf).clamp(150.0, 4000.0);
                    if (centre - self.tips_at).abs() > 0.05 * centre {
                        self.tips_at = centre;
                        self.tips.retune(Biquad::bandpass(centre, 0.9));
                    }
                }
                None => self.rotor_level.send(0.0, 0.3),
            }
            for (strength, pan) in said.steps.drain(..) {
                let pitch = 1800.0 + 900.0 * self.noise.unit();
                self.steps.push(Step {
                    age: 0,
                    strength: strength.clamp(0.02, 1.5),
                    pan,
                    thud: Wave::default(),
                    grit: Biquad::bandpass(pitch, 0.9),
                    rustle: Biquad::bandpass(380.0 + 200.0 * self.noise.unit(), 0.7),
                });
            }
        }
        // Breathing follows effort, and recovers from it slowly.
        let target = 0.12 + 0.88 * self.work;
        self.exertion += (target - self.exertion) * (dt / if target > self.exertion { 6.0 } else { 25.0 }).min(1.0);
        // Gusts.
        self.gust_in -= dt;
        if self.gust_in <= 0.0 {
            self.gust_in = 2.0 + 6.0 * self.noise.unit();
            let gust = self.noise.unit();
            self.wind_level.send(self.said.wind * (0.35 + 0.65 * gust), 1.5);
        }
        self.steps.retain(|s| s.age < (0.5 * RATE) as u32);
    }

    /// One breath after another; faster and deeper the harder you work.
    fn breathe(&mut self, noise: f32) -> f32 {
        let b = &mut self.breath;
        b.at += 1.0 / RATE;
        if b.at >= b.period {
            let rate = 11.0 + 22.0 * self.exertion;
            *b = Breath { at: 0.0, period: 60.0 / rate, depth: 0.02 + 0.07 * self.exertion, band: b.band.clone(), level: b.level, out: false };
            b.band.retune(Biquad::bandpass(1300.0, 1.1));
        }
        let (tin, tex) = (0.4 * b.period, 0.5 * b.period);
        if b.at < 0.8 * tin {
            b.level.send(b.depth, 0.3 * tin);
        } else if b.at < tin {
            b.level.send(0.0, 0.15 * tin);
        } else if b.at < tin + 0.7 * tex {
            if !b.out {
                // Out is lower than in.
                b.out = true;
                b.band.retune(Biquad::bandpass(760.0, 1.1));
            }
            b.level.send(0.8 * b.depth, 0.25 * tex);
        } else {
            b.level.send(0.0, 0.2 * tex);
        }
        // The band takes a third of white noise's level: make it up.
        3.0 * b.band.run(noise) * b.level.step()
    }
}

/// A boot on soil: the strike, carried up the leg as a dead low thud that
/// the regolith soaks up; the grit under the sole, which in vacuum only
/// the boot can carry and which air carries too; and the suit's layers
/// working as the leg swings through. Returns what goes through the suit
/// and what goes through the air.
fn footfall(step: &mut Step, noise: f32) -> (f32, f32) {
    let t = step.age as f32 / RATE;
    step.age += 1;
    let k = step.strength;
    // Exponential ramps, as an oscillator and a gain would be scheduled.
    let ramp = |from: f32, to: f32, t: f32, over: f32| from * (to / from).powf((t / over).clamp(0.0, 1.0));
    let thud = if t < 0.25 {
        let pitch = ramp(75.0, 38.0, t, 0.14);
        let level = if t < 0.006 { ramp(0.0001, 0.62 * k, t, 0.006) } else { ramp(0.62 * k, 0.0001, t - 0.006, 0.214) };
        step.thud.sine(pitch) * level
    } else {
        0.0
    };
    let grit = if t < 0.12 {
        let level = if t < 0.004 { ramp(0.0001, 0.2 * k, t, 0.004) } else { ramp(0.2 * k, 0.0001, t - 0.004, 0.096) };
        2.5 * step.grit.run(noise) * level
    } else {
        0.0
    };
    // The swing of the other leg, after the strike: a soft swell.
    let swell = ((t - 0.08) / 0.4).clamp(0.0, 1.0);
    let rustle = 2.5 * step.rustle.run(noise) * 0.022 * k.min(0.6) * (swell * std::f32::consts::PI).sin().powi(2);
    (thud + grit + rustle, grit)
}

impl Voice for Suit {
    fn frame(&mut self) -> [f32; 2] {
        if self.clock.is_multiple_of(BLOCK) {
            self.listen();
        }
        self.clock = self.clock.wrapping_add(1);
        // Two noises, partly shared: the hiss is all round the head, not
        // at a point between the ears.
        let (a, b) = (self.noise.white(), self.noise.white());
        let n = [0.8 * a + 0.6 * b, 0.8 * a - 0.6 * b];
        let (mut suit, mut air) = ([0.0f32; 2], [0.0f32; 2]);

        // The backpack: fans, the gas they move through the helmet, and a
        // water pump. Its hum wanders a little, as motors do.
        let wander = 1.0 + 0.004 * self.drift.sine(0.13);
        let hum = 0.010 * self.hum[0].sine(118.0 * wander) + 0.004 * self.hum[1].sine(236.0 * wander) + 0.002 * self.hum[2].sine(59.0 * wander);
        for side in 0..2 {
            suit[side] += 0.06 * self.fan[side].run(n[side]) + 0.014 * self.flow[side].run(n[side]) + hum;
        }

        let breath = self.breathe(a);
        suit[0] += breath;
        suit[1] += breath;

        for step in &mut self.steps {
            let (through, outside) = footfall(step, b);
            let (left, right) = (1.0 - 0.35 * step.pan.max(0.0), 1.0 + 0.35 * step.pan.min(0.0));
            suit[0] += through * left;
            suit[1] += through * right;
            air[0] += outside * left;
            air[1] += outside * right;
        }

        // Jets: cold nitrogen through the backpack frame.
        let jets = self.jets.step();
        if jets > 1e-5 {
            let high = self.jet[0].run(a);
            let hiss = self.jet[1].run(high) * jets;
            suit[0] += hiss;
            suit[1] += hiss;
        }

        // The rover: four hub motors and their harmonic drives, through
        // the seat, and through the air where there is any.
        let (motor_level, gear_level, pitch) = (self.motor_level.step(), self.gear_level.step(), self.motor_pitch.step());
        if motor_level > 1e-5 {
            let motor = self.motor_band.run(self.motor.saw(55.0 + 240.0 * pitch)) * motor_level;
            let gear = self.gear.triangle(170.0 + 740.0 * pitch) * gear_level;
            for side in 0..2 {
                suit[side] += motor + gear;
                air[side] += motor;
            }
        }

        // A rotorcraft: the blades' passing, a buzz with a thump under it
        // at the same rate, and the hiss off the tips. Only the air brings
        // it.
        let (rotor_level, rotor_pitch, pan) = (self.rotor_level.step(), self.rotor_pitch.step(), self.rotor_pan.step());
        if rotor_level > 1e-5 && rotor_pitch > 1.0 {
            let buzz = self.rotor_band.run(self.rotor.saw(rotor_pitch));
            let thump = self.thump.sine(rotor_pitch);
            let hiss = 2.0 * self.tips.run(b);
            let sound = (0.7 * buzz + 0.9 * thump + 0.35 * hiss) * rotor_level;
            air[0] += sound * (1.0 - 0.5 * pan.max(0.0));
            air[1] += sound * (1.0 + 0.5 * pan.min(0.0));
        }

        // Wind, where there is air to move.
        let wind = self.wind_level.step();
        if wind > 1e-5 {
            for side in 0..2 {
                air[side] += 4.0 * self.wind[side].run(n[side]) * wind;
            }
        }

        let (carries, master) = (self.carries.step(), self.master.step());
        let mut out = [0.0; 2];
        for side in 0..2 {
            let through = self.helmet[side].run(self.ring[side].run(suit[side]));
            let outside = self.outside[side].run(air[side]) * carries;
            out[side] = soften(LEVEL * master * (through + outside));
        }
        out
    }
}

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        if !wanted() {
            return;
        }
        app.insert_resource(Line(Arc::new(Mutex::new(Said::default()))))
            .insert_resource(CabinLine(Arc::new(Mutex::new(crate::cabin::Aboard::default()))))
            .add_systems(Startup, start)
            .add_systems(Update, tell.after(Striding));
    }
}

/// What the command module's cabin should sound (see `cabin`): the
/// Earthrise scene writes to it.
#[derive(Resource, Clone)]
pub struct CabinLine(pub Arc<Mutex<crate::cabin::Aboard>>);

fn start(mut commands: Commands, mut synths: ResMut<Assets<Synth>>, line: Res<Line>, cabin: Res<CabinLine>) {
    play(&mut commands, &mut synths, Suit::new(line.0.clone()));
    play(&mut commands, &mut synths, crate::cabin::Cabin::new(cabin.0.clone()));
}

/// Once a frame: what the voice should know.
fn tell(
    line: Res<Line>,
    current: Res<Current>,
    world: Res<WorldDef>,
    muted: Res<Muted>,
    driving: Option<Res<Driving>>,
    player: Single<(&Player, &Transform)>,
    rover: Option<Single<(&Vehicle, &Transform), With<Rover>>>,
    craft: Option<Single<(&Rotorcraft, &Transform), With<Helicopter>>>,
    mut falls: MessageReader<Footfall>,
    stroll: Option<Res<crate::stroll::Stroll>>,
    earthrise: Option<Res<crate::earthrise::Earthrise>>,
) {
    let (player, ear) = player.into_inner();
    let Ok(mut said) = line.0.lock() else { return };
    said.medium = Medium::of(&current.0);
    // Down to a whisper while the picture is black, never quite silent.
    said.level = 1.0 - 0.85 * stroll.as_ref().map_or(0.0, |s| s.dark);
    // Aboard the command module there is no suit: the cabin speaks.
    if earthrise.is_some_and(|e| e.on) {
        said.level = 0.0;
    }
    let aboard = driving.is_some_and(|d| d.aboard);
    // Work: nothing standing, a quarter at a walk, all of it at a lope
    // uphill.
    let pace = Vec2::new(player.velocity.x, player.velocity.z).length();
    let climb = (player.velocity.y / pace.max(0.1)).clamp(0.0, 0.5);
    said.work = if aboard || player.flying || !player.grounded { 0.0 } else { ((pace / world.walker.run.max(0.01)).powi(2) * 0.7 + 1.2 * climb * (pace / world.walker.walk.max(0.01)).min(1.0)).min(1.0) };
    said.jets = player.thrust;
    said.rover = match (&rover, aboard) {
        (Some(rover), true) => Some((rover.0.forward_speed(rover.1).abs() / 3.6, rover.0.throttle.abs())),
        _ => None,
    };
    // Loading noise goes with the thrust and the tips' speed; the pressure
    // falls off as one over the distance.
    said.rotor = craft.and_then(|craft| {
        let (rc, pose) = craft.into_inner();
        let blades = rc.spec.rotors.first().map_or(2, |r| r.blades) as f32;
        let bpf = rc.rpm / 60.0 * blades;
        let thrust: f32 = rc.thrust.iter().sum();
        let source = (thrust * rc.tip_mach()).sqrt() + 0.4 * (rc.rpm / rc.spec.rpm);
        let to = pose.translation - ear.translation;
        let distance = to.length().max(0.8);
        let pan = to.normalize_or_zero().dot(*ear.right());
        (bpf > 1.0).then_some((bpf, (0.05 * source / distance).min(0.12), pan))
    });
    for fall in falls.read() {
        if !muted.0 {
            said.steps.push((fall.strength, if fall.left { -1.0 } else { 1.0 }));
        }
    }
    // A stalled audio thread must not pile steps up.
    let n = said.steps.len();
    if n > 8 {
        said.steps.drain(..n - 8);
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Renders a minute to a 16-bit WAV file, with no audio device and no
/// window: twenty seconds standing, twenty-five walking, five at a lope,
/// ten standing again. For measuring.
pub fn render(path: &str, body: &Body) -> std::io::Result<()> {
    let line = Arc::new(Mutex::new(Said { medium: Medium::of(body), level: 1.0, ..default() }));
    let mut suit = Suit::new(line.clone());
    let seconds = 60;
    let frames = seconds * RATE as usize;
    let walker = crate::suit::walker(body.gravity, body.grip, body.drag);
    let mut data = Vec::with_capacity(frames * 2);
    let (mut next_step, mut left) = (20.0f32, false);
    for k in 0..frames {
        let t = k as f32 / RATE;
        let pace = if (20.0..45.0).contains(&t) { walker.walk } else if (45.0..50.0).contains(&t) { walker.run } else { 0.0 };
        if k % BLOCK as usize == 0 {
            let mut said = line.lock().unwrap();
            said.work = (pace / walker.run).powi(2) * 0.7;
            if pace > 0.0 && t >= next_step {
                next_step = t + crate::gait::step_length(pace, body.gravity) / pace;
                said.steps.push((if pace > walker.walk { 0.55 } else { 0.3 }, if left { -1.0 } else { 1.0 }));
                left = !left;
            }
        }
        for sample in suit.frame() {
            data.push((sample.clamp(-1.0, 1.0) * 32767.0) as i16);
        }
    }
    std::fs::write(path, tranquility::record::wav_bytes(&data))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    /// Left-channel level over a second of the suit standing on a body,
    /// from the third second (after the fade in).
    fn standing(body: &Body) -> f32 {
        let line = Arc::new(Mutex::new(Said { medium: Medium::of(body), level: 1.0, ..default() }));
        let mut suit = Suit::new(line);
        let all: Vec<f32> = (0..4 * RATE as usize).map(|_| suit.frame()[0]).collect();
        rms(&all[3 * RATE as usize..])
    }

    fn body(name: &str) -> Body {
        crate::bodies::all().into_iter().find(|b| b.name == name).unwrap()
    }

    #[test]
    fn vacuum_carries_nothing_and_air_what_it_can() {
        assert_eq!(Medium::of(&body("Moon")).carries, 0.0);
        assert_eq!(Medium::of(&body("Moon")).wind, 0.0);
        let mars = Medium::of(&body("Mars"));
        assert!((0.08..0.2).contains(&mars.carries), "{mars:?}");
        assert!(mars.wind > 0.1);
        assert_eq!(Medium::of(&body("Titan")).carries, 1.0);
    }

    #[test]
    fn the_suit_is_never_silent_and_never_loud() {
        let level = standing(&body("Moon"));
        assert!((0.01..0.1).contains(&level), "{level}");
    }

    /// A rotor's level at the ear on a body, after the air has had it: two
    /// suits alike in everything else, and what differs between them.
    fn rotor_on(name: &str) -> f32 {
        let said = || Arc::new(Mutex::new(Said { medium: Medium::of(&body(name)), level: 1.0, ..default() }));
        let (with, without) = (said(), said());
        with.lock().unwrap().rotor = Some((84.6, 0.05, 0.0));
        let (mut a, mut b) = (Suit::new(with), Suit::new(without));
        let diff: Vec<f32> = (0..3 * RATE as usize).map(|_| a.frame()[0] - b.frame()[0]).collect();
        rms(&diff[RATE as usize..])
    }

    /// Nothing on an airless body, faint on Mars, plain on Titan.
    #[test]
    fn rotors_are_heard_as_the_air_carries_them() {
        let (moon, mars, titan) = (rotor_on("Moon"), rotor_on("Mars"), rotor_on("Titan"));
        assert!(moon.abs() < 1e-4, "{moon}");
        assert!(mars > 0.001 && titan > 3.0 * mars, "Mars {mars}, Titan {titan}");
    }

    #[test]
    fn a_footfall_is_heard_over_the_backpack() {
        let line = Arc::new(Mutex::new(Said { level: 1.0, ..default() }));
        let mut suit = Suit::new(line.clone());
        let quiet: Vec<f32> = (0..3 * RATE as usize).map(|_| suit.frame()[0]).collect();
        line.lock().unwrap().steps.push((0.3, -1.0));
        let step: Vec<f32> = (0..(0.25 * RATE) as usize).map(|_| suit.frame()[0]).collect();
        let before = rms(&quiet[2 * RATE as usize..]);
        assert!(rms(&step) > 2.0 * before, "{} against {before}", rms(&step));
        assert!(step.iter().all(|s| s.abs() < 0.9));
    }
}
