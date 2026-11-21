//! Recording a video, offline: `TRANQUILITY_RECORD=out.mp4` (and
//! `TRANQUILITY_RECORD_SECONDS=600`, ten minutes by default) runs the app on
//! a clock of exactly a sixtieth of a second a frame, however long a frame
//! really takes, and hands every frame to `ffmpeg` (H.264, as good as it
//! gets: CRF 16) as it is drawn. Sound made by [`crate::sound`] voices is
//! pulled from them a frame's worth at a time instead of played, and put
//! beside the picture at the end. Recording starts once the ground has
//! streamed in; the app quits when the video is done.
//!
//! So a slow machine makes a smooth video: it only takes longer. The
//! quality tier is High unless `TRANQUILITY_QUALITY` says otherwise. The
//! picture is drawn off the screen, into an image of exactly
//! `TRANQUILITY_SIZE` (1920x1080 unless given), whatever the window and the
//! screen are: the window stays black meanwhile.
//!
//! The clock waits for the ground. When the view turns or moves into ground
//! not yet streamed in (a telephoto lens swinging, finer chunks to mesh),
//! the frame is held: the clock stops, nothing is taken, and the camera
//! keeps to where the last frame was taken from, until every chunk wanted
//! is there (or [`HOLD_LIMIT`] has passed). Then the clock goes on, and the
//! frame after a hold moves from the one before it as any other does, so
//! motion blur and the antialiasing's history carry on unbroken.
//!
//! For the best picture, `TRANQUILITY_SUPERSAMPLE=2` draws every frame at
//! twice the size each way and has `ffmpeg` scale it down (Lanczos): four
//! samples a pixel, edges and fine ground without shimmer.
//! `TRANQUILITY_RECORD_CRF` sets the encoder's quality (lower is better:
//! 16 by default, 0 lossless). Ultra (`--ultra`) makes the
//! defaults 2 and 12, with x264's slower preset. The drawn frame may be
//! at most 8192 pixels each way.

use std::{
    io::Write,
    process::{Child, ChildStdin, Command, Stdio},
    time::Duration,
};

use bevy::{
    camera::RenderTarget,
    prelude::*,
    render::{
        render_resource::TextureFormat,
        view::screenshot::{Screenshot, ScreenshotCaptured},
    },
    time::TimeUpdateStrategy,
    ui::{IsDefaultUiCamera, UiScale},
};

/// Frames a second, of the picture and of the clock.
pub const FPS: u32 = 60;

/// The longest a frame is held for the ground, in seconds of real time.
const HOLD_LIMIT: f64 = 30.0;

/// The recording in progress.
#[derive(Resource)]
pub struct Recorder {
    path: String,
    seconds: f32,
    /// Frames asked for and frames handed to the encoder.
    asked: u32,
    written: u32,
    /// Seconds since the ground was all there.
    settled: Option<f32>,
    /// Whether frames are being taken now: the sound's voices are pulled
    /// only while they are.
    pub rolling: bool,
    /// Sound, interleaved stereo at [`crate::sound::RATE`], as it was pulled.
    pub sound: Vec<i16>,
    encoder: Option<(Child, ChildStdin)>,
    finished: bool,
    /// The picture's size, from the first frame, and whether a later one
    /// has differed.
    size: Option<(u32, u32)>,
    resized: bool,
    /// The video's size, when the frames are drawn larger and scaled down
    /// to it; the encoder's CRF and preset.
    out: UVec2,
    crf: u32,
    preset: &'static str,
    /// Where the camera was when the last frame was taken, in render
    /// space; when the present hold began (real time); holds so far, and
    /// how many updates they took.
    shot_from: Option<GlobalTransform>,
    holding: Option<f64>,
    holds: u32,
    held: u32,
}

/// A recording cut short (the window closed, Ctrl-C, the GPU lost) is
/// finished with what it has, so the frames already drawn come out as a
/// video with its sound and not as a stray intermediate file.
impl Drop for Recorder {
    fn drop(&mut self) {
        if !self.finished && self.encoder.is_some() {
            self.finished = true;
            warn!("record: stopped after {:.1} s of {}; finishing what there is", self.written as f32 / FPS as f32, self.seconds);
            // The sound as far as the picture goes.
            let samples = (self.written as usize * crate::sound::RATE as usize / FPS as usize) * 2;
            self.sound.truncate(samples);
            finish(self);
        }
    }
}

/// Whether this run records a video.
pub fn recording() -> bool {
    std::env::var_os("TRANQUILITY_RECORD").is_some()
}

pub(crate) fn add(app: &mut App) {
    let Ok(path) = std::env::var("TRANQUILITY_RECORD") else { return };
    let seconds = crate::tools::env_f32("TRANQUILITY_RECORD_SECONDS").unwrap_or(600.0);
    let size = std::env::var("TRANQUILITY_SIZE")
        .ok()
        .and_then(|s| s.split_once('x').and_then(|(w, h)| Some(UVec2::new(w.trim().parse().ok()?, h.trim().parse().ok()?))))
        .unwrap_or(UVec2::new(1920, 1080));
    let ultra = crate::quality::Quality::asked() == Some(crate::quality::Quality::Ultra);
    let wanted = std::env::var("TRANQUILITY_SUPERSAMPLE").ok().and_then(|v| v.trim().parse::<u32>().ok()).unwrap_or(if ultra { 2 } else { 1 }).max(1);
    // As much as fits in the largest texture a GPU is sure to allow.
    let factor = (1..=wanted).rev().find(|f| size.x * f <= 8192 && size.y * f <= 8192).unwrap_or(1);
    if factor != wanted {
        warn!("record: {}x{} drawn {wanted} times over is past 8192 pixels; drawing it {factor} times over", size.x, size.y);
    }
    let out = size;
    let size = size * factor;
    let crf = std::env::var("TRANQUILITY_RECORD_CRF").ok().and_then(|v| v.trim().parse().ok()).unwrap_or(if ultra { 12 } else { 16 });
    let preset = if ultra { "slower" } else { "slow" };
    let image = {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        images.add(Image::new_target_texture(size.x, size.y, TextureFormat::Rgba8UnormSrgb, None))
    };
    // Text laid out for a window about 720 high keeps its proportions.
    app.insert_resource(UiScale(size.y as f32 / 720.0))
        .insert_resource(Canvas(image))
        .add_systems(Update, aim_cameras);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / FPS as f64)))
        .insert_resource(Recorder {
            path,
            seconds,
            asked: 0,
            written: 0,
            settled: None,
            rolling: false,
            sound: Vec::new(),
            encoder: None,
            finished: false,
            size: None,
            resized: false,
            out,
            crf,
            preset,
            shot_from: None,
            holding: None,
            holds: 0,
            held: 0,
        })
        .add_systems(Last, roll.in_set(Rolling));
}

/// Where frames are asked for: the sound is pulled just before, so frame
/// and sound stay in step.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rolling;

/// The image the recording is drawn into.
#[derive(Resource)]
struct Canvas(Handle<Image>);

/// Cameras draw into the canvas, and the text with them.
fn aim_cameras(mut commands: Commands, canvas: Res<Canvas>, cameras: Query<Entity, Added<Camera3d>>) {
    for camera in &cameras {
        commands.entity(camera).insert((RenderTarget::from(canvas.0.clone()), IsDefaultUiCamera));
    }
}

fn roll(
    mut commands: Commands,
    time: Res<Time>,
    real: Res<Time<Real>>,
    terrain: Res<crate::terrain::Chunks>,
    canvas: Res<Canvas>,
    mut recorder: ResMut<Recorder>,
    mut clock: ResMut<TimeUpdateStrategy>,
    mut camera: Query<(&mut Transform, &mut GlobalTransform), With<Camera3d>>,
    mut shifted: MessageReader<crate::origin::OriginShifted>,
    mut exit: MessageWriter<AppExit>,
) {
    let t = time.elapsed_secs();
    for shift in shifted.read() {
        if let Some(from) = &mut recorder.shot_from {
            *from = GlobalTransform::from(from.compute_transform().with_translation(from.translation() - shift.delta));
        }
    }
    if recorder.settled.is_none() && t > 2.0 && terrain.is_settled() {
        recorder.settled = Some(t);
        info!("record: the ground is in; recording {} s to {}", recorder.seconds, recorder.path);
    }
    let Some(settled) = recorder.settled else { return };
    // A moment for the antialiasing and the sky to settle.
    let due = recorder.asked < (recorder.seconds * FPS as f32) as u32;
    let rolling = t > settled + 1.0 && due;
    let mut camera = camera.iter_mut().next();
    if rolling && !terrain.is_settled() {
        let now = real.elapsed_secs_f64();
        let since = *recorder.holding.get_or_insert(now);
        if let Some(from) = recorder.shot_from
            && now - since < HOLD_LIMIT
        {
            if since == now {
                recorder.holds += 1;
                *clock = TimeUpdateStrategy::ManualDuration(Duration::ZERO);
            }
            recorder.held += 1;
            recorder.rolling = false;
            // Drawn from where the last frame was, so the next frame's
            // motion is measured from there; the transform is marked so
            // the next update draws the camera from where it really is
            // again, before it is held once more.
            if let Some((transform, global)) = &mut camera {
                **global = from;
                transform.set_changed();
            }
            return;
        }
        if now - since >= HOLD_LIMIT {
            warn!("record: the ground did not come in {HOLD_LIMIT} s; going on without it");
        }
    }
    if recorder.holding.take().is_some() {
        *clock = TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / FPS as f64));
    }
    recorder.rolling = rolling;
    if recorder.rolling {
        recorder.asked += 1;
        recorder.shot_from = camera.map(|(_, global)| *global);
        commands.spawn(Screenshot::image(canvas.0.clone())).observe(take);
        if recorder.asked.is_multiple_of(FPS * 10) {
            info!("record: {} s of {}; held {} times for the ground, {} updates", recorder.asked / FPS, recorder.seconds, recorder.holds, recorder.held);
        }
    } else if !due && recorder.written >= recorder.asked && !recorder.finished {
        recorder.finished = true;
        finish(&mut recorder);
        exit.write(AppExit::Success);
    }
}

/// A frame drawn: to the encoder, which is started on the first one, when
/// the size is known.
fn take(captured: On<ScreenshotCaptured>, mut recorder: ResMut<Recorder>) {
    let Ok(image) = captured.image.clone().try_into_dynamic() else {
        error!("record: a frame in a format that cannot be read");
        recorder.written += 1;
        return;
    };
    let frame = image.to_rgba8();
    let (width, height) = (frame.width(), frame.height());
    let mut pixels = frame.into_raw();
    // Every frame the size of the first, whatever the window did since:
    // cropped, or padded with black.
    if let Some(size) = recorder.size
        && (width, height) != size
    {
        pixels = fit(&pixels, (width, height), size);
        if !recorder.resized {
            recorder.resized = true;
            warn!("record: the window changed size; frames are fitted to the first one's");
        }
    }
    let size = *recorder.size.get_or_insert((width, height));
    if recorder.encoder.is_none() {
        let video = format!("{}.video.mp4", recorder.path);
        // Drawn larger than the video: scaled down to it, sharply.
        let scale = if (size.0, size.1) != (recorder.out.x, recorder.out.y) {
            format!("scale={}:{}:flags=lanczos,", recorder.out.x, recorder.out.y)
        } else {
            String::new()
        };
        let filter = format!("{scale}crop=trunc(iw/2)*2:trunc(ih/2)*2");
        let crf = recorder.crf.to_string();
        let preset = recorder.preset;
        let size = format!("{}x{}", size.0, size.1);
        let rate = FPS.to_string();
        let mut command = Command::new("ffmpeg");
        // In a group of its own: Ctrl-C in the terminal is for the app,
        // which then finishes the video; the encoder must not die first.
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
        let child = command
            .args(["-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", &size, "-r", &rate, "-i", "-"])
            // Even dimensions, for 4:2:0.
            .args(["-vf", &filter, "-c:v", "libx264", "-preset", preset, "-crf", &crf, "-pix_fmt", "yuv420p", &video])
            .stdin(Stdio::piped())
            .spawn();
        match child {
            Ok(mut child) => {
                let stdin = child.stdin.take().expect("piped");
                recorder.encoder = Some((child, stdin));
            }
            Err(e) => {
                error!("record: cannot start ffmpeg ({e}); nothing will be recorded");
                recorder.seconds = 0.0;
            }
        }
    }
    if let Some((_, stdin)) = &mut recorder.encoder
        && let Err(e) = stdin.write_all(&pixels)
    {
        error!("record: ffmpeg stopped taking frames: {e}");
    }
    recorder.written += 1;
}

/// RGBA pixels of one size, cropped or padded with black to another.
fn fit(pixels: &[u8], from: (u32, u32), to: (u32, u32)) -> Vec<u8> {
    let mut out = vec![0u8; (to.0 * to.1 * 4) as usize];
    for pixel in out.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
    let (w, h) = (from.0.min(to.0) as usize * 4, from.1.min(to.1) as usize);
    for row in 0..h {
        let src = row * from.0 as usize * 4;
        let dst = row * to.0 as usize * 4;
        out[dst..dst + w].copy_from_slice(&pixels[src..src + w]);
    }
    out
}

/// Closes the picture, writes the sound, and puts the two together.
fn finish(recorder: &mut Recorder) {
    let Some((mut child, stdin)) = recorder.encoder.take() else { return };
    drop(stdin);
    let _ = child.wait();
    let video = format!("{}.video.mp4", recorder.path);
    let mut inputs = vec!["-i".to_string(), video.clone()];
    if !recorder.sound.is_empty() {
        let wav = format!("{}.sound.wav", recorder.path);
        if let Err(e) = std::fs::write(&wav, wav_bytes(&recorder.sound)) {
            error!("record: cannot write the sound: {e}");
        } else {
            inputs.extend(["-i".to_string(), wav]);
        }
    }
    let mut command = Command::new("ffmpeg");
    command.args(["-y", "-loglevel", "error"]).args(&inputs).args(["-c:v", "copy"]);
    if inputs.len() > 2 {
        command.args(["-c:a", "aac", "-b:a", "256k", "-shortest"]);
    }
    match command.arg(&recorder.path).status() {
        Ok(status) if status.success() => {
            let _ = std::fs::remove_file(&video);
            let _ = std::fs::remove_file(format!("{}.sound.wav", recorder.path));
            info!("record: {} frames to {}", recorder.written, recorder.path);
        }
        _ => error!("record: putting picture and sound together failed; they are in {video} and {}.sound.wav", recorder.path),
    }
}

/// 16-bit stereo PCM, as a WAV file.
pub fn wav_bytes(samples: &[i16]) -> Vec<u8> {
    let rate = crate::sound::RATE as u32;
    let data = samples.len() as u32 * 2;
    let mut file = Vec::with_capacity(data as usize + 44);
    file.extend(b"RIFF");
    file.extend((36 + data).to_le_bytes());
    file.extend(b"WAVEfmt ");
    file.extend(16u32.to_le_bytes());
    file.extend(1u16.to_le_bytes());
    file.extend(2u16.to_le_bytes());
    file.extend(rate.to_le_bytes());
    file.extend((rate * 4).to_le_bytes());
    file.extend(4u16.to_le_bytes());
    file.extend(16u16.to_le_bytes());
    file.extend(b"data");
    file.extend(data.to_le_bytes());
    for s in samples {
        file.extend(s.to_le_bytes());
    }
    file
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_are_fitted_to_the_first_size() {
        let red = [255u8, 0, 0, 255].repeat(4 * 2);
        let bigger = fit(&red, (4, 2), (5, 3));
        assert_eq!(bigger.len(), 5 * 3 * 4);
        assert_eq!(&bigger[0..4], &[255, 0, 0, 255]);
        assert_eq!(&bigger[16..20], &[0, 0, 0, 255]);
        let smaller = fit(&red, (4, 2), (3, 1));
        assert_eq!(smaller, [255u8, 0, 0, 255].repeat(3));
    }
}
