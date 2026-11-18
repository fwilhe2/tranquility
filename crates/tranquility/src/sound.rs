//! Sound, synthesised: nothing is loaded and nothing decoded. A world's
//! app writes a [`Voice`] (a function from time to a stereo sample, built
//! from the noise, filters and envelopes here) and hands it to [`play`];
//! Bevy's audio pulls samples from it on the audio thread for as long as
//! the app runs. What the voice should be doing (how fast the walker
//! breathes, when a boot came down) reaches it through whatever the app
//! shares with it, typically an `Arc<Mutex<…>>` read once per block.
//!
//! `M` mutes. Runs that only take a picture or a measurement
//! (`TRANQUILITY_SCREENSHOT`, `TRANQUILITY_BENCH`) and `TRANQUILITY_SOUND=off`
//! stay silent: see [`wanted`].

use std::{f32::consts::TAU, sync::Mutex, time::Duration};

use bevy::{
    audio::{AddAudioSource, ChannelCount, Decodable, SampleRate, Source},
    prelude::*,
};

/// Samples a second, of every voice.
pub const RATE: f32 = 44_100.0;

/// Something that sounds. Called on the audio thread, once per sample.
pub trait Voice: Send + 'static {
    /// The next sample, left and right, about −1 to 1.
    fn frame(&mut self) -> [f32; 2];
}

/// A voice as an audio asset. It plays once: the first player to start it
/// takes the voice, any later one gets silence.
#[derive(Asset, TypePath)]
pub struct Synth(Mutex<Option<Box<dyn Voice>>>);

impl Synth {
    pub fn new(voice: impl Voice) -> Self {
        Self(Mutex::new(Some(Box::new(voice))))
    }
}

/// Starts a voice; it sounds until the entity returned is despawned. While
/// a video is being recorded it goes on the recording's tape instead.
pub fn play(commands: &mut Commands, synths: &mut Assets<Synth>, voice: impl Voice) -> Entity {
    #[cfg(not(target_arch = "wasm32"))]
    if crate::record::recording() {
        let voice: Box<dyn Voice> = Box::new(voice);
        commands.queue(move |world: &mut World| {
            if let Some(tape) = world.get_resource::<Tape>()
                && let Ok(mut voices) = tape.0.lock()
            {
                voices.push(voice);
            }
        });
        return commands.spawn_empty().id();
    }
    commands.spawn(AudioPlayer(synths.add(Synth::new(voice)))).id()
}

/// The voices of a recording, pulled a frame's worth at a time.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource, Default)]
struct Tape(Mutex<Vec<Box<dyn Voice>>>);

#[cfg(not(target_arch = "wasm32"))]
fn pull(tape: Res<Tape>, mut recorder: ResMut<crate::record::Recorder>) {
    if !recorder.rolling {
        return;
    }
    let Ok(mut voices) = tape.0.lock() else { return };
    let n = (RATE as u32 / crate::record::FPS) as usize;
    recorder.sound.reserve(2 * n);
    for _ in 0..n {
        let mut frame = [0.0f32; 2];
        for voice in voices.iter_mut() {
            let [l, r] = voice.frame();
            frame[0] += l;
            frame[1] += r;
        }
        for s in frame {
            recorder.sound.push((s.clamp(-1.0, 1.0) * 32767.0) as i16);
        }
    }
}

/// Whether this run should make any sound at all.
pub fn wanted() -> bool {
    let set = |name| std::env::var_os(name).is_some();
    !set("TRANQUILITY_SCREENSHOT") && !set("TRANQUILITY_BENCH") && std::env::var("TRANQUILITY_SOUND").map_or(true, |v| v != "off")
}

pub struct Stream {
    voice: Option<Box<dyn Voice>>,
    /// The right sample of a frame whose left has gone out.
    right: Option<f32>,
}

impl Iterator for Stream {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if let Some(right) = self.right.take() {
            return Some(right);
        }
        let [left, right] = self.voice.as_mut().map_or([0.0; 2], |v| v.frame());
        self.right = Some(right);
        Some(left)
    }
}

impl Source for Stream {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::new(2).unwrap()
    }

    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(RATE as u32).unwrap()
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl Decodable for Synth {
    type Decoder = Stream;

    fn decoder(&self) -> Stream {
        Stream { voice: self.0.lock().ok().and_then(|mut v| v.take()), right: None }
    }
}

/// `M`: sound off and on.
#[derive(Resource, Default)]
pub struct Muted(pub bool);

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<Synth>().init_resource::<Muted>().add_systems(Update, mute);
        #[cfg(not(target_arch = "wasm32"))]
        if crate::record::recording() {
            app.init_resource::<Tape>().add_systems(Last, pull.before(crate::record::Rolling));
        }
    }
}

fn mute(keys: Res<ButtonInput<KeyCode>>, mut muted: ResMut<Muted>, mut hud: ResMut<crate::hud::Hud>, mut sinks: Query<&mut AudioSink>) {
    if keys.just_pressed(KeyCode::KeyM) {
        muted.0 = !muted.0;
        hud.say(if muted.0 { "sound off" } else if wanted() { "sound on" } else { "no sound in this run" });
    }
    for mut sink in &mut sinks {
        if muted.is_changed() || sink.is_added() {
            if muted.0 {
                sink.mute();
            } else {
                sink.unmute();
            }
        }
    }
}

/// White noise, −1 to 1.
#[derive(Clone)]
pub struct Noise(u32);

impl Noise {
    pub fn new(seed: u32) -> Self {
        Self(seed.wrapping_mul(747_796_405) | 1)
    }

    pub fn white(&mut self) -> f32 {
        // xorshift32
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / 8_388_608.0 - 1.0
    }

    /// 0 to 1.
    pub fn unit(&mut self) -> f32 {
        self.white() * 0.5 + 0.5
    }
}

/// A second-order filter (the Audio EQ Cookbook's), one channel.
#[derive(Clone, Default)]
pub struct Biquad {
    b: [f32; 3],
    a: [f32; 2],
    x: [f32; 2],
    y: [f32; 2],
}

impl Biquad {
    fn with(b: [f32; 3], a: [f32; 3]) -> Self {
        Self { b: b.map(|v| v / a[0]), a: [a[1] / a[0], a[2] / a[0]], ..default() }
    }

    fn turn(frequency: f32, q: f32) -> (f32, f32) {
        let w = TAU * (frequency / RATE).clamp(0.0001, 0.49);
        (w.cos(), w.sin() / (2.0 * q))
    }

    pub fn lowpass(frequency: f32, q: f32) -> Self {
        let (c, alpha) = Self::turn(frequency, q);
        Self::with([(1.0 - c) / 2.0, 1.0 - c, (1.0 - c) / 2.0], [1.0 + alpha, -2.0 * c, 1.0 - alpha])
    }

    pub fn highpass(frequency: f32, q: f32) -> Self {
        let (c, alpha) = Self::turn(frequency, q);
        Self::with([(1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0], [1.0 + alpha, -2.0 * c, 1.0 - alpha])
    }

    /// Unit gain at the centre.
    pub fn bandpass(frequency: f32, q: f32) -> Self {
        let (c, alpha) = Self::turn(frequency, q);
        Self::with([alpha, 0.0, -alpha], [1.0 + alpha, -2.0 * c, 1.0 - alpha])
    }

    /// A bump (or a dip) of `gain` decibels round a frequency.
    pub fn peaking(frequency: f32, q: f32, gain: f32) -> Self {
        let (c, alpha) = Self::turn(frequency, q);
        let a = 10f32.powf(gain / 40.0);
        Self::with([1.0 + alpha * a, -2.0 * c, 1.0 - alpha * a], [1.0 + alpha / a, -2.0 * c, 1.0 - alpha / a])
    }

    /// The same filter somewhere else, keeping what is ringing in it: for
    /// a band that moves.
    pub fn retune(&mut self, to: Biquad) {
        (self.b, self.a) = (to.b, to.a);
    }

    pub fn run(&mut self, x: f32) -> f32 {
        let y = self.b[0] * x + self.b[1] * self.x[0] + self.b[2] * self.x[1] - self.a[0] * self.y[0] - self.a[1] * self.y[1];
        self.x = [x, self.x[0]];
        self.y = [y, self.y[0]];
        y
    }
}

/// A value that goes where it is sent with a time constant, never at once:
/// levels and pitches changed this way do not click.
#[derive(Clone, Copy, Default)]
pub struct Glide {
    pub now: f32,
    pub to: f32,
    rate: f32,
}

impl Glide {
    pub fn new(value: f32, seconds: f32) -> Self {
        Self { now: value, to: value, rate: Self::rate(seconds) }
    }

    fn rate(seconds: f32) -> f32 {
        1.0 - (-1.0 / (seconds.max(1e-4) * RATE)).exp()
    }

    /// Head for `to`, covering most of the way in `seconds`.
    pub fn send(&mut self, to: f32, seconds: f32) {
        (self.to, self.rate) = (to, Self::rate(seconds));
    }

    pub fn step(&mut self) -> f32 {
        self.now += (self.to - self.now) * self.rate;
        self.now
    }
}

/// A sine (or, through [`Wave::saw`] and [`Wave::triangle`], something
/// buzzier) whose pitch may change as it goes.
#[derive(Clone, Copy, Default)]
pub struct Wave(f32);

impl Wave {
    fn turn(&mut self, frequency: f32) -> f32 {
        self.0 = (self.0 + frequency / RATE).fract();
        self.0
    }

    pub fn sine(&mut self, frequency: f32) -> f32 {
        (self.turn(frequency) * TAU).sin()
    }

    pub fn saw(&mut self, frequency: f32) -> f32 {
        2.0 * self.turn(frequency) - 1.0
    }

    pub fn triangle(&mut self, frequency: f32) -> f32 {
        4.0 * (self.turn(frequency) - 0.5).abs() - 1.0
    }
}

/// Rounds off what goes over full scale instead of clipping it.
pub fn soften(x: f32) -> f32 {
    if x.abs() < 0.6 { x } else { x.signum() * (0.6 + 0.4 * ((x.abs() - 0.6) / 0.4).tanh()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level(filter: &mut Biquad, frequency: f32) -> f32 {
        let mut wave = Wave::default();
        let mut sum = 0.0;
        for k in 0..20_000 {
            let y = filter.run(wave.sine(frequency));
            if k >= 10_000 {
                sum += y * y;
            }
        }
        (sum / 10_000.0).sqrt() * 2f32.sqrt()
    }

    #[test]
    fn filters_pass_and_stop_what_they_should() {
        assert!((level(&mut Biquad::lowpass(1000.0, 0.707), 100.0) - 1.0).abs() < 0.02);
        assert!(level(&mut Biquad::lowpass(1000.0, 0.707), 8000.0) < 0.03);
        assert!(level(&mut Biquad::highpass(1000.0, 0.707), 100.0) < 0.02);
        assert!((level(&mut Biquad::bandpass(700.0, 1.0), 700.0) - 1.0).abs() < 0.02);
        assert!((level(&mut Biquad::peaking(900.0, 1.2, 6.0), 900.0) - 2.0).abs() < 0.05);
    }

    #[test]
    fn a_stream_interleaves_left_and_right() {
        struct Sides;
        impl Voice for Sides {
            fn frame(&mut self) -> [f32; 2] {
                [-1.0, 1.0]
            }
        }
        let stream = Synth::new(Sides).decoder();
        assert_eq!(stream.take(4).collect::<Vec<_>>(), [-1.0, 1.0, -1.0, 1.0]);
    }
}
