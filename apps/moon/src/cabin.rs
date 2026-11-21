//! What a crew heard inside the Apollo command module, generated, for the
//! Earthrise (`earthrise.rs`). In shirtsleeves, not suits: the cabin's own
//! air carries everything.
//!
//! - The cabin fans and the suit-circuit compressors: a broad rush of air,
//!   the noisiest thing aboard (crews often switched the fans off to sleep);
//! - the spacecraft's AC power, 400 Hz from the inverters, a thin whine
//!   under it, and the glycol pump of the coolant loop, low;
//! - the reaction control jets of the service module: each firing a hard
//!   bang carried through the structure, a short crack with a thump in the
//!   hull behind it. A roll starts and ends with a run of them, and while
//!   it coasts the autopilot's deadband fires one now and then;
//! - the Hasselblad 500 EL: the shutter's clack, then its motor winding
//!   the film on, ending as it cocks; and a film magazine clicked on.
//!
//! The crew's words are on the screen, not synthesised. Levels are set by
//! ear against the suit (`sound.rs`); nothing is a recording.

use std::sync::{Arc, Mutex};

use tranquility::sound::{Biquad, Glide, Noise, RATE, Voice, Wave, soften};

/// Read what the scene said this often, in samples.
const BLOCK: u32 = 256;

/// Something that happens once, to be heard.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Clunk {
    /// A jet: −1 left to 1 right, and how hard (about 1).
    Jet { pan: f32, strength: f32 },
    /// The camera taking a frame, and winding on.
    Shutter,
    /// A film magazine clicked onto the camera.
    Magazine,
}

/// What the scene tells the cabin.
#[derive(Default)]
pub struct Aboard {
    /// 0–1: how much of the cabin to hear (none while not aboard).
    pub level: f32,
    pub clunks: Vec<Clunk>,
}

/// A clunk sounding.
struct Sounding {
    clunk: Clunk,
    age: u32,
    filters: [Biquad; 3],
    wave: Wave,
}

impl Sounding {
    fn new(clunk: Clunk) -> Self {
        let filters = match clunk {
            Clunk::Jet { .. } => [Biquad::lowpass(2400.0, 0.7), Biquad::bandpass(140.0, 1.4), Biquad::highpass(300.0, 0.7)],
            Clunk::Shutter => [Biquad::bandpass(3200.0, 1.6), Biquad::bandpass(1150.0, 2.5), Biquad::bandpass(5200.0, 3.0)],
            Clunk::Magazine => [Biquad::bandpass(2300.0, 2.0), Biquad::bandpass(700.0, 3.0), Biquad::default()],
        };
        Self { clunk, age: 0, filters, wave: Wave::default() }
    }

    fn length(&self) -> f32 {
        match self.clunk {
            Clunk::Jet { .. } => 0.5,
            Clunk::Shutter => 1.1,
            Clunk::Magazine => 0.4,
        }
    }

    /// The next sample, left and right.
    fn frame(&mut self, noise: f32) -> [f32; 2] {
        let t = self.age as f32 / RATE;
        self.age += 1;
        // A sharp edge and a fall, as a struck thing makes.
        let hit = |t: f32, attack: f32, decay: f32| if t < 0.0 { 0.0 } else if t < attack { t / attack } else { (-(t - attack) / decay).exp() };
        match self.clunk {
            Clunk::Jet { pan, strength } => {
                // The crack of the valve opening and the plume's first
                // shock, through the hull; then the hull ringing low.
                let crack = self.filters[0].run(noise) * hit(t, 0.002, 0.035);
                let body = 3.0 * self.filters[1].run(noise) * hit(t, 0.004, 0.11);
                let thump = self.wave.sine(58.0 - 14.0 * t.min(0.3)) * hit(t, 0.003, 0.09);
                // The firing itself, tens of milliseconds of hiss.
                let burn = if t < 0.06 { 0.25 * self.filters[2].run(noise) } else { 0.0 };
                let s = 0.65 * strength * (0.5 * crack + 0.35 * body + 0.4 * thump + burn);
                [s * (1.0 - 0.4 * pan.max(0.0)), s * (1.0 + 0.4 * pan.min(0.0))]
            }
            Clunk::Shutter => {
                // Mirror and shutter: two clacks 40 ms apart.
                let clack = self.filters[0].run(noise) * (hit(t, 0.001, 0.012) + 0.7 * hit(t - 0.04, 0.001, 0.016));
                let knock = self.filters[1].run(noise) * (hit(t, 0.002, 0.03) + hit(t - 0.04, 0.002, 0.03));
                // The motor winding on: a small DC motor through a gear
                // train, rising as it spins up and falling as it cocks.
                let (from, to) = (0.12, 0.78);
                let wind = if (from..to).contains(&t) {
                    let u = (t - from) / (to - from);
                    let envelope = (u / 0.08).min(1.0) * ((1.0 - u) / 0.1).min(1.0);
                    let pitch = 150.0 + 70.0 * u.min(0.3) / 0.3 - 30.0 * (u - 0.8).max(0.0) / 0.2;
                    let motor = self.wave.saw(pitch);
                    envelope * (0.05 * motor + 0.6 * self.filters[2].run(noise) * (0.5 + 0.5 * motor))
                } else {
                    0.0
                };
                let cock = self.filters[0].run(noise) * 0.5 * hit(t - to, 0.001, 0.01);
                let s = 0.9 * clack + 0.5 * knock + 0.35 * wind + cock;
                [s, 0.9 * s]
            }
            Clunk::Magazine => {
                let s = self.filters[0].run(noise) * (hit(t, 0.001, 0.008) + 0.8 * hit(t - 0.09, 0.001, 0.01)) + 0.6 * self.filters[1].run(noise) * hit(t - 0.09, 0.002, 0.03);
                [0.6 * s, 0.7 * s]
            }
        }
    }
}

/// The cabin, as a voice.
pub struct Cabin {
    line: Arc<Mutex<Aboard>>,
    noise: Noise,
    clock: u32,
    level: Glide,
    /// The fans: a rush, and the blades' tone in it.
    rush: [Biquad; 2],
    blades: [Biquad; 2],
    hiss: [Biquad; 2],
    inverter: [Wave; 2],
    pump: Wave,
    drift: Wave,
    sounding: Vec<Sounding>,
}

impl Cabin {
    pub fn new(line: Arc<Mutex<Aboard>>) -> Self {
        let pair = |f: Biquad| [f.clone(), f];
        Self {
            line,
            noise: Noise::new(1968),
            clock: 0,
            level: Glide::new(0.0, 1.0),
            rush: pair(Biquad::lowpass(900.0, 0.6)),
            blades: pair(Biquad::bandpass(330.0, 6.0)),
            hiss: pair(Biquad::bandpass(2800.0, 0.7)),
            inverter: [Wave::default(); 2],
            pump: Wave::default(),
            drift: Wave::default(),
            sounding: Vec::new(),
        }
    }

    fn listen(&mut self) {
        if let Ok(mut aboard) = self.line.try_lock() {
            if (self.level.to - aboard.level).abs() > 1e-3 {
                self.level.send(aboard.level, 0.8);
            }
            for clunk in aboard.clunks.drain(..) {
                self.sounding.push(Sounding::new(clunk));
            }
        }
        self.sounding.retain(|s| (s.age as f32 / RATE) < s.length());
    }
}

impl Voice for Cabin {
    fn frame(&mut self) -> [f32; 2] {
        if self.clock.is_multiple_of(BLOCK) {
            self.listen();
        }
        self.clock = self.clock.wrapping_add(1);
        let level = self.level.step();
        if level < 1e-4 && self.sounding.is_empty() {
            return [0.0; 2];
        }
        let (a, b) = (self.noise.white(), self.noise.white());
        let n = [0.8 * a + 0.6 * b, 0.8 * a - 0.6 * b];
        let wander = 1.0 + 0.003 * self.drift.sine(0.07);
        let hum = 0.006 * self.inverter[0].sine(400.0) + 0.002 * self.inverter[1].sine(800.0) + 0.008 * self.pump.sine(47.0 * wander);
        let mut out = [0.0f32; 2];
        for side in 0..2 {
            let air = 0.2 * self.rush[side].run(n[side]) + 0.09 * self.blades[side].run(n[side]) + 0.045 * self.hiss[side].run(n[side]);
            out[side] = level * (air + hum);
        }
        for s in &mut self.sounding {
            let [l, r] = s.frame(b);
            out[0] += l;
            out[1] += r;
        }
        // Held under 0.85 of full scale, so the encoder's overshoot on a
        // run of jets stays below it.
        out.map(|x| 0.85 * soften(1.6 * x))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    fn listen(cabin: &mut Cabin, seconds: f32) -> Vec<f32> {
        (0..(seconds * RATE) as usize).map(|_| cabin.frame()[0]).collect()
    }

    #[test]
    fn the_cabin_hums_and_a_jet_bangs_over_it() {
        let line = Arc::new(Mutex::new(Aboard { level: 1.0, ..Default::default() }));
        let mut cabin = Cabin::new(line.clone());
        let quiet = rms(&listen(&mut cabin, 3.0)[RATE as usize * 2..]);
        assert!(quiet > 0.005 && quiet < 0.1, "cabin {quiet}");
        line.lock().unwrap().clunks.push(Clunk::Jet { pan: 0.0, strength: 1.0 });
        let bang = listen(&mut cabin, 0.05);
        let peak = bang.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak > 4.0 * quiet && peak < 1.0, "jet {peak} over {quiet}");
        line.lock().unwrap().clunks.push(Clunk::Shutter);
        let click = listen(&mut cabin, 0.03);
        assert!(rms(&click) > 2.0 * quiet, "shutter {}", rms(&click));
    }

    #[test]
    fn away_from_the_cabin_there_is_nothing() {
        let line = Arc::new(Mutex::new(Aboard::default()));
        let mut cabin = Cabin::new(line);
        assert!(listen(&mut cabin, 1.0).iter().all(|s| *s == 0.0));
    }
}
