//! What the overlay and the F1 menu say about the picture: the upscaler, the
//! render and output sizes, and frame generation (AMD's or DLSS's, with the
//! frames shown). Noted by the runner, the upscalers and the present.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};

/// The upscaler that serves the title's FidelityFX calls: "game" (its own
/// console build), "FSR 3.1" (AMD's PC build) or "DLSS".
static UPSCALER: Mutex<&'static str> = Mutex::new("game");
/// The scene's size over the output's when no upscaling said it (the title's
/// own: 2/3 on both consoles), as f32 bits.
static SCALE: AtomicU32 = AtomicU32::new(0x3f2a_aaab);
/// The last upscaling's render size, `width << 32 | height` (0: none seen).
static RENDER: AtomicU64 = AtomicU64::new(0);
/// The swap chain's size, likewise.
static OUTPUT: AtomicU64 = AtomicU64::new(0);
/// Frames shown a second while AMD's frame generation is on, times 100.
static AMD_SHOWN_X100: AtomicU64 = AtomicU64::new(0);

/// The upscaler (set by the runner before the title starts).
pub fn note_upscaler(name: &'static str) {
    *UPSCALER.lock().unwrap_or_else(|e| e.into_inner()) = name;
}

/// The render scale the image was patched to (the runner).
pub fn note_scale(scale: f32) {
    SCALE.store(scale.to_bits(), Relaxed);
}

/// An upscaling's render size.
pub fn note_render(render: [u32; 2]) {
    RENDER.store(u64::from(render[0]) << 32 | u64::from(render[1]), Relaxed);
}

pub(crate) fn note_output(width: u32, height: u32) {
    OUTPUT.store(u64::from(width) << 32 | u64::from(height), Relaxed);
}

pub(crate) fn note_amd_shown(per_second: f64) {
    AMD_SHOWN_X100.store((per_second * 100.0) as u64, Relaxed);
}

fn pair(v: u64) -> Option<[u32; 2]> {
    (v != 0).then_some([(v >> 32) as u32, v as u32])
}

/// The picture as the overlay shows it.
#[derive(Clone, Debug, Default)]
pub(crate) struct Picture {
    pub console: &'static str,
    pub upscaler: &'static str,
    pub render: Option<[u32; 2]>,
    pub output: Option<[u32; 2]>,
    /// Frame generation running: its maker, and the frames shown a second
    /// for `fps` rendered.
    pub framegen: Option<&'static str>,
}

pub(crate) fn picture() -> Picture {
    let output = pair(OUTPUT.load(Relaxed));
    let scale = f32::from_bits(SCALE.load(Relaxed));
    let render = pair(RENDER.load(Relaxed)).or_else(|| output.map(|o| o.map(|v| (v as f32 * scale) as u32)));
    let framegen = if crate::streamline::shown_per_frame() > 1 {
        Some("DLSS")
    } else if crate::framegen::running() {
        Some("AMD")
    } else {
        None
    };
    Picture {
        console: match crate::console::current() {
            crate::console::Console::SeriesS => "Series S",
            crate::console::Console::SeriesX => "Series X",
        },
        upscaler: *UPSCALER.lock().unwrap_or_else(|e| e.into_inner()),
        render,
        output,
        framegen,
    }
}

/// Frames shown a second for `fps` rendered: with frame generation on, its
/// count; else `fps`.
pub(crate) fn shown(fps: f64) -> f64 {
    match crate::streamline::shown_per_frame() {
        1 if crate::framegen::running() => AMD_SHOWN_X100.load(Relaxed) as f64 / 100.0,
        1 => fps,
        per => fps * f64::from(per),
    }
}
