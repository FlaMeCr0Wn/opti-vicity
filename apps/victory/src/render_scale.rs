//! The scene drawn at a scale of the output through the title's FidelityFX
//! upscaler path (`VICTORY_RENDER_SCALE`): the retail image's renderer
//! selection and the upscaler's render size are patched before it is mapped.
//! The stock title draws a Series S scene at 2/3 of its 1920x1080 output
//! (1280x720, "quality"), and on Series X uses its own temporal pass.
//!
//! Smaller scales need AMD's PC upscaler (`VICTORY_FIDELITYFX_DIR`,
//! `victory_hle::modules::fidelityfx`): the title's own shipping FSR takes
//! only 1280x720 in and 1920x1080 out.
//!
//! The patch is trustno1's (trustno1-coder/VIctory-NVIDIA, `apps/victory/src/
//! native_1440p.rs`, `prepare_upscaler`, after DexNom's fork), GPL-3.0-or-
//! later. Every patched instruction is checked against the stock bytes
//! first; another build is refused, not patched.

use anyhow::{Context, Result, ensure};
use victory_loader::PreparedImage;

const RETAIL_BUILD: &str = "b99f876b74d4a77f03af5f878223b27bd647b5c35a79d5decd9f716504239e93";

/// The render size over the output size, by FidelityFX's names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Native,
    Quality,
    Balanced,
    Performance,
    UltraPerformance,
}

impl Mode {
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        Some(match name.trim().to_ascii_lowercase().as_str() {
            "native" | "nativeaa" | "dlaa" => Mode::Native,
            "quality" => Mode::Quality,
            "balanced" => Mode::Balanced,
            "performance" => Mode::Performance,
            "ultraperformance" | "ultra-performance" | "ultra_performance" => Mode::UltraPerformance,
            _ => return None,
        })
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Mode::Native => "native",
            Mode::Quality => "quality",
            Mode::Balanced => "balanced",
            Mode::Performance => "performance",
            Mode::UltraPerformance => "ultraperformance",
        }
    }

    /// FidelityFX's ratios.
    pub(crate) fn scale(self) -> f32 {
        match self {
            Mode::Native => 1.0,
            Mode::Quality => 2.0 / 3.0,
            Mode::Balanced => 1.0 / 1.7,
            Mode::Performance => 0.5,
            Mode::UltraPerformance => 1.0 / 3.0,
        }
    }

    /// The FidelityFX quality mode, which the title notes.
    fn ffx_mode(self) -> u8 {
        match self {
            Mode::Native => 0,
            Mode::Quality => 1,
            Mode::Balanced => 2,
            Mode::Performance => 3,
            Mode::UltraPerformance => 4,
        }
    }

    /// The scene's size for `output` (the title truncates).
    pub(crate) fn render_size(self, output: [u32; 2]) -> [u32; 2] {
        output.map(|v| (v as f32 * self.scale()) as u32)
    }
}

/// The title's renderer selection: `mov eax, 6; cmovne eax, ecx` (the
/// title's own temporal pass, or on Series S with the upscaler there the
/// FidelityFX path, 7).
const RENDERER_SELECTION: u32 = 0x007e_9421;
const STOCK_RENDERER_SELECTION: [u8; 8] = [0xb8, 6, 0, 0, 0, 0x0f, 0x45, 0xc1];
/// The FidelityFX path's render size: the console check's result (Series S)
/// is noted as the FidelityFX quality mode and picks the scaled branch
/// (`movzx eax, al; mov [r14+0x90], eax; test al, al; jne`), whose scale
/// multiplies the output size.
const UPSCALER_SIZING: u32 = 0x028e_f69a;
const STOCK_UPSCALER_SIZING: [u8; 14] = [0x0f, 0xb6, 0xc0, 0x41, 0x89, 0x86, 0x90, 0, 0, 0, 0x84, 0xc0, 0x75, 0x0a];
/// The native branch's load of 1.0, and the scaled branch's of the scale.
const NATIVE_LOAD: u32 = 0x028e_f6a8;
const STOCK_NATIVE_LOAD: [u8; 10] = [0xc5, 0xfa, 0x10, 0x05, 0x28, 0x78, 0x6d, 0x03, 0xeb, 0x08];
const SCALED_LOAD: u32 = 0x028e_f6b2;
const STOCK_SCALED_LOAD: [u8; 8] = [0xc5, 0xfa, 0x10, 0x05, 0xa2, 0xe2, 0x86, 0x03];
/// The scale the scaled branch loads (shared with other code: not written).
const UPSCALER_SCALE: u32 = 0x0615_d95c;
/// The Series X temporal pass's scene scale, paired with the upscaler's.
const TEMPORAL_SCALE: u32 = 0x028f_1e7e;

/// Makes the title render through its FidelityFX upscaler path (renderer 7,
/// as Series S does) with the scene at `mode`'s scale of the output. The
/// FidelityFX quality mode the title notes is `mode`'s; the temporal scene
/// scale is set to the same scale, as the stock Series X image pairs them
/// (2/3 each). Nothing is written if one field differs from the stock bytes.
pub(crate) fn prepare(image: &mut PreparedImage, mode: Mode) -> Result<()> {
    ensure!(
        image.original_hash.to_hex().as_str() == RETAIL_BUILD,
        "the render scale is only known for the retail build {RETAIL_BUILD}"
    );
    ensure!(*field::<8>(image, RENDERER_SELECTION)? == STOCK_RENDERER_SELECTION, "unexpected renderer selection");
    ensure!(*field::<14>(image, UPSCALER_SIZING)? == STOCK_UPSCALER_SIZING, "unexpected upscaler render size selection");
    ensure!(*field::<10>(image, NATIVE_LOAD)? == STOCK_NATIVE_LOAD, "unexpected upscaler native scale load");
    ensure!(*field::<8>(image, SCALED_LOAD)? == STOCK_SCALED_LOAD, "unexpected upscaler scaled load");
    ensure!(*field::<4>(image, UPSCALER_SCALE)? == (2.0f32 / 3.0).to_le_bytes(), "unexpected upscaler scale");
    ensure!(
        *field::<4>(image, TEMPORAL_SCALE)? == (2.0f32 / 3.0).to_le_bytes(),
        "unexpected temporal scene scale"
    );
    // Renderer 7 every time it is asked (the console check has run).
    *field::<8>(image, RENDERER_SELECTION)? = [0xb8, 7, 0, 0, 0, 0x90, 0x90, 0x90];
    // `mov eax, mode; mov [r14+0x90], eax; jmp scaled`.
    *field::<14>(image, UPSCALER_SIZING)? = [0xb8, mode.ffx_mode(), 0, 0, 0, 0x41, 0x89, 0x86, 0x90, 0, 0, 0, 0xeb, 0x0a];
    if mode != Mode::Quality {
        // The jump leaves the native branch unreached: its load's operand
        // becomes a literal of ours, which the scaled branch loads.
        let literal = NATIVE_LOAD + 4;
        *field::<4>(image, literal)? = mode.scale().to_le_bytes();
        let displacement = literal as i32 - (SCALED_LOAD as i32 + 8);
        *field::<4>(image, SCALED_LOAD + 4)? = displacement.to_le_bytes();
    }
    *field::<4>(image, TEMPORAL_SCALE)? = mode.scale().to_le_bytes();
    Ok(())
}

fn field<const N: usize>(image: &mut PreparedImage, rva: u32) -> Result<&mut [u8; N]> {
    let offset = image.info.rva_to_file_offset(rva).context("upscaler field missing")? as usize;
    image
        .bytes
        .get_mut(offset..offset + N)
        .context("incomplete upscaler field")?
        .try_into()
        .context("invalid upscaler field length")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_sizes() {
        assert_eq!(Mode::from_name(" Performance "), Some(Mode::Performance));
        assert_eq!(Mode::from_name("x"), None);
        assert_eq!(Mode::Quality.render_size([1920, 1080]), [1280, 720]);
        assert_eq!(Mode::Balanced.render_size([1920, 1080]), [1129, 635]);
        assert_eq!(Mode::Performance.render_size([1920, 1080]), [960, 540]);
        for mode in [Mode::Native, Mode::Quality, Mode::Balanced, Mode::Performance, Mode::UltraPerformance] {
            assert_eq!(Mode::from_name(mode.name()), Some(mode));
        }
    }

    /// With the retail executable named by `VICTORY_DISPLAY_PROFILE_TEST_IMAGE`
    /// (not part of the repository): the scaled branch loads the mode's
    /// scale and the noted mode is its FidelityFX mode.
    #[test]
    fn patches_the_upscaler_path() {
        let Some(path) = std::env::var_os("VICTORY_DISPLAY_PROFILE_TEST_IMAGE") else {
            return;
        };
        let bytes = std::fs::read(path).unwrap();
        for mode in [Mode::Native, Mode::Quality, Mode::Balanced, Mode::Performance, Mode::UltraPerformance] {
            let mut image = victory_loader::prepare(&bytes).unwrap();
            prepare(&mut image, mode).unwrap();
            assert_eq!(*field::<8>(&mut image, RENDERER_SELECTION).unwrap(), [0xb8, 7, 0, 0, 0, 0x90, 0x90, 0x90]);
            let sizing = *field::<14>(&mut image, UPSCALER_SIZING).unwrap();
            assert_eq!(sizing[1], mode.ffx_mode());
            assert_eq!(UPSCALER_SIZING + 14 + 0x0a, SCALED_LOAD);
            let disp = i32::from_le_bytes(*field::<4>(&mut image, SCALED_LOAD + 4).unwrap());
            let loaded = (SCALED_LOAD + 8).wrapping_add_signed(disp);
            assert_eq!(f32::from_le_bytes(*field::<4>(&mut image, loaded).unwrap()), mode.scale());
            assert_eq!(*field::<4>(&mut image, UPSCALER_SCALE).unwrap(), (2.0f32 / 3.0).to_le_bytes());
            assert!(prepare(&mut image, mode).is_err());
        }
    }
}
