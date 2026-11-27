//! Why the game closed with an error, read from its log, in words the
//! launcher can show: a short line for the status and a longer text with
//! what to try and the log's own lines (the engine's last error, the
//! exception and where it happened, a panic, a lost graphics device, a
//! full paging file).

/// What the launcher says about a run that ended with an error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    /// One line for the status (no "the game closed" in front).
    pub short: String,
    /// The whole notice: the cause, what to try and the log's lines.
    pub detail: String,
    /// The programs whose pipeline the driver was making when the game
    /// stopped ([`left_out_programs`]): the launcher leaves them out next time.
    pub leave_out: Vec<String>,
}

/// The programs (16-digit hashes) of the host pipeline the stopped thread
/// was making, from the engine's "while making the host pipeline of
/// [compute 240cb516c1af0bc3]" under the last unhandled exception.
pub fn left_out_programs(log: &str) -> Vec<String> {
    let lines: Vec<&str> = log.lines().collect();
    let Some(at) = lines.iter().rposition(|s| s.contains("unhandled exception 0x")) else {
        return Vec::new();
    };
    lines[at + 1..]
        .iter()
        .map(|s| s.trim())
        .take_while(|s| !s.starts_with('#'))
        .filter_map(|s| s.strip_prefix("while making the host pipeline of "))
        .flat_map(|s| s.split(|c: char| !c.is_ascii_alphanumeric()))
        .filter(|w| w.len() == 16 && w.bytes().all(|b| b.is_ascii_hexdigit()))
        .map(str::to_ascii_lowercase)
        .collect()
}

/// A text in the user's language.
fn l(pt: bool, pt_text: &str, en_text: &str) -> String {
    if pt { pt_text } else { en_text }.to_owned()
}

/// The log without the terminal's colour codes.
fn plain(log: &str) -> String {
    let mut out = String::with_capacity(log.len());
    let mut chars = log.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            // Up to the code's final letter.
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// A Windows exception (or exit) code by name.
fn exception_name(code: u32, pt: bool) -> String {
    match code {
        0x8000_0003 => l(
            pt,
            "parada interna do jogo (breakpoint): o jogo encontrou algo que não esperava e parou de propósito",
            "the game's own stop (breakpoint): it met something it did not expect and stopped on purpose",
        ),
        0xc000_0005 => l(pt, "acesso inválido à memória", "invalid memory access"),
        0xc000_0409 => l(pt, "encerramento de segurança (fail-fast)", "security stop (fail-fast)"),
        0xc000_00fd => l(pt, "estouro de pilha", "stack overflow"),
        0xc000_0017 | 0xc000_012d => l(pt, "memória insuficiente", "out of memory"),
        0xc000_001d => l(pt, "instrução que o processador não tem", "an instruction the CPU does not have"),
        0xe06d_7363 => l(pt, "exceção de C++ não tratada", "unhandled C++ exception"),
        _ => l(pt, "erro do Windows", "Windows error"),
    }
}

/// Where a known stop of the title is, what it means and what to try.
fn known_place(place: &str, pt: bool) -> Option<(String, String)> {
    // AMD's shader compiler, making a pipeline of the title's programs.
    if place.to_ascii_lowercase().starts_with("amdxc64.dll+") {
        return Some((
            l(pt, "o compilador de shaders do driver AMD travou ao preparar um shader do jogo", "AMD's driver shader compiler crashed making one of the game's shaders"),
            l(
                pt,
                "Use a versão mais nova do opti-vicity (ela evita o caso conhecido, amdxc64+0x861b9f) e atualize o driver da AMD. Se continuar, mande o game.log (botão Logs).",
                "Use the newest opti-vicity (it avoids the known case, amdxc64+0x861b9f) and update the AMD driver. If it goes on, send game.log (the Logs button).",
            ),
        ));
    }
    // NVIDIA's D3D12 driver.
    if place.to_ascii_lowercase().starts_with("nvwgf2umx.dll+") {
        return Some((
            l(pt, "o driver da NVIDIA travou", "NVIDIA's driver crashed"),
            l(
                pt,
                "Atualize o driver da NVIDIA e tente com a geração de quadros desligada. Se continuar, mande o game.log (botão Logs).",
                "Update the NVIDIA driver and try with frame generation off. If it goes on, send game.log (the Logs button).",
            ),
        ));
    }
    // The image's name carries the build; the offset is what counts.
    let offset = place.rsplit_once('+').map(|(_, o)| o.to_ascii_lowercase())?;
    if !place.contains("victory-image-b99f876b74d4a77f") {
        return None;
    }
    match offset.as_str() {
        // 2026-11-27: the title's allocator, out of room for a scene target
        // (the Native render scale; on Series S 0x882000 bytes).
        "0x2219650" => Some((
            l(pt, "o jogo não conseguiu reservar memória para uma imagem da cena", "the game could not reserve memory for a scene image"),
            l(
                pt,
                "Use a Resolução Qualidade (ou menor), desmarque Series X, feche programas pesados e deixe um arquivo de paginação grande (32 GB de RAM recomendado).",
                "Use the Quality resolution (or lower), untick Series X, close heavy programs and keep a large paging file (32 GB of RAM recommended).",
            ),
        )),
        // VIctory's README: a missing appdata0.rpf, before the first window.
        "0x2237178" => Some((
            l(pt, "falta um arquivo do jogo (appdata0.rpf)", "a game file is missing (appdata0.rpf)"),
            l(
                pt,
                "Copie o appdata0.rpf do mesmo pacote do jogo para a pasta do game_scarlett_master.exe.",
                "Copy appdata0.rpf from the same package of the game into the folder of game_scarlett_master.exe.",
            ),
        )),
        _ => None,
    }
}

/// Explains a run that ended with exit `code`, from its log.
pub fn explain(log: &str, code: Option<i32>, pt: bool) -> Report {
    let log = plain(log);
    let lines: Vec<&str> = log.lines().collect();
    let mut technical: Vec<String> = Vec::new();
    let mut cause: Option<String> = None;
    let mut advice: Option<String> = None;

    // The fatal exception: the last "unhandled exception" and its stack.
    if let Some(at) = lines.iter().rposition(|s| s.contains("unhandled exception 0x")) {
        let line = lines[at].trim();
        technical.push(line.trim_start_matches("VIctory: ").to_owned());
        if let Some(doing) =
            lines[at + 1..].iter().map(|s| s.trim()).take_while(|s| !s.starts_with('#')).find(|s| s.starts_with("while "))
        {
            technical.push(format!("  {doing}"));
        }
        technical.extend(
            lines[at + 1..]
                .iter()
                .map(|s| s.trim())
                .filter(|s| s.starts_with('#'))
                .take(6)
                .map(|s| format!("  {s}")),
        );
        let mut words = line.split_whitespace();
        let hex = words.clone().find_map(|w| w.strip_prefix("0x")).and_then(|h| u32::from_str_radix(h, 16).ok());
        let place = words.find(|w| w.contains('+')).unwrap_or_default();
        if let Some((what, try_this)) = known_place(place, pt) {
            cause = Some(what);
            advice = Some(try_this);
        } else if let Some(hex) = hex {
            cause = Some(exception_name(hex, pt));
        }
    }

    // A panic in the engine: "panicked at <place>:" and its message.
    if cause.is_none()
        && let Some(at) = lines.iter().rposition(|s| s.contains("panicked at"))
    {
        let message = lines.get(at + 1).map(|s| s.trim()).filter(|s| !s.is_empty()).unwrap_or("");
        technical.push(lines[at].trim().to_owned());
        if !message.is_empty() {
            technical.push(format!("  {message}"));
        }
        cause = Some(format!("{}: {message}", l(pt, "erro interno do opti-vicity", "internal opti-vicity error")));
    }

    // The engine's own error ("Error: ..." and its causes).
    if let Some(at) = lines.iter().rposition(|s| s.starts_with("Error: ")) {
        let message = lines[at].trim_start_matches("Error: ").trim().to_owned();
        technical.push(lines[at].trim().to_owned());
        technical.extend(
            lines[at + 1..]
                .iter()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty() && *s != "Caused by:")
                .take(4)
                .map(|s| format!("  {s}")),
        );
        if cause.is_none() {
            cause = Some(message);
        }
    }

    // The graphics card was lost (driver reset, hang or removal).
    if lines.iter().any(|s| s.contains("device-removed data") || s.contains("DXGI_ERROR_DEVICE") || s.contains("device removed")) {
        if cause.is_none() {
            cause = Some(l(pt, "a placa de vídeo parou de responder (driver reiniciado)", "the graphics card stopped answering (driver reset)"));
        }
        advice.get_or_insert_with(|| {
            l(
                pt,
                "Atualize o driver da placa, desligue a geração de quadros e tente o Upscaler \"Do jogo\".",
                "Update the graphics driver, turn frame generation off and try the game's own upscaler.",
            )
        });
    }

    // Windows' commit limit (the paging file is full).
    let commit_full = log.contains("os error 1455") || log.contains("paging file");
    if commit_full {
        if cause.is_none() {
            cause = Some(l(pt, "o Windows ficou sem memória virtual (arquivo de paginação cheio)", "Windows ran out of virtual memory (paging file full)"));
        }
        advice.get_or_insert_with(|| {
            l(
                pt,
                "Aumente o arquivo de paginação (por exemplo 32 GB ou mais) e feche programas pesados.",
                "Make the paging file larger (for example 32 GB or more) and close heavy programs.",
            )
        });
    }
    // Allocations refused with E_OUTOFMEMORY: the driver crash that often
    // follows is a consequence, so the memory is named as the cause.
    let refused = lines.iter().filter(|s| s.contains("0x8007000E") || s.contains("0x8007000e")).count();
    if refused > 0 && !commit_full {
        cause = Some(l(pt, "a memória acabou (da placa de vídeo ou do PC) e o jogo não conseguiu mais reservar memória", "memory ran out (the graphics card's or the PC's) and the game could no longer reserve any"));
        advice = Some(l(
            pt,
            "Escolha Memória da placa \"8 GB (texturas menores)\", use uma Resolução menor, desmarque Series X, feche programas pesados (navegador, Discord) e deixe um arquivo de paginação grande.",
            "Pick card memory \"8 GB (smaller textures)\", use a lower Resolution, untick Series X, close heavy programs (browser, Discord) and keep a large paging file.",
        ));
        if let Some(first) = lines.iter().find(|s| s.contains("0x8007000E") || s.contains("0x8007000e")) {
            technical.push(format!("{refused}x {}", first.trim()));
        }
    }
    // The PC's memory was close to its limit during the run (under 4 GB of
    // free virtual memory; above that the warning is routine).
    let pressure = lines
        .iter()
        .rev()
        .find(|s| s.contains("host memory pressure"))
        .and_then(|s| {
            s.split_whitespace()
                .find_map(|w| w.strip_prefix("available_commit_mib="))
                .and_then(|v| v.parse::<u64>().ok())
        })
        .filter(|mib| *mib < 4096);

    let cause = cause.unwrap_or_else(|| match code {
        Some(c) => format!("{} {:#x}", exception_name(c as u32, pt), c as u32),
        None => l(pt, "o processo foi encerrado", "the process was ended"),
    });
    let mut detail = format!("{}: {cause}.", l(pt, "Causa", "Cause"));
    if let Some(mib) = pressure {
        detail.push_str(&format!(
            "\n\n{} {:.1} GB.",
            l(pt, "A memória do PC estava quase no limite durante o jogo: memória virtual livre", "The PC's memory was close to its limit during the run: free virtual memory"),
            mib as f64 / 1024.0
        ));
    }
    let leave_out = left_out_programs(&log);
    if !leave_out.is_empty() {
        advice = Some(format!(
            "{} ({}). {}",
            l(
                pt,
                "Abra o jogo de novo: o opti-vicity vai deixar de fora o programa que travou o driver",
                "Start the game again: opti-vicity will leave out the program that stopped the driver",
            ),
            leave_out.join(", "),
            l(
                pt,
                "Pode faltar algum efeito na tela. Para trazê-lo de volta, apague leave-out.txt na pasta do cache. Atualizar o driver também ajuda.",
                "Some effect may be missing on screen. To bring it back, delete leave-out.txt in the cache folder. Updating the driver helps too.",
            )
        ));
    }
    let advice = advice.unwrap_or_else(|| {
        l(
            pt,
            "Tente com Upscaler \"Do jogo\" e Ger. quadros \"Desligada\". Se continuar, peça ajuda mandando o game.log (botão Logs).",
            "Try the game's own upscaler with frame generation off. If it goes on, ask for help with game.log (the Logs button).",
        )
    });
    detail.push_str(&format!("\n\n{}: {advice}", l(pt, "O que tentar", "What to try")));
    if let Some(c) = code {
        technical.insert(0, format!("{} {c} ({:#x})", l(pt, "código de saída", "exit code"), c as u32));
    }
    if !technical.is_empty() {
        detail.push_str(&format!("\n\n{}:\n{}", l(pt, "Do log", "From the log"), technical.join("\n")));
    }
    detail.push_str(&format!(
        "\n\n{}",
        l(
            pt,
            "Ctrl+C copia este aviso. Para pedir ajuda, mande também o game.log (botão Logs).",
            "Ctrl+C copies this notice. To ask for help, also send game.log (the Logs button).",
        )
    ));
    Report { short: cause, detail, leave_out }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RADEON: &str = "frame rate: our clock 16.667 ms\n\
        \x1b[2m2026\x1b[0m \x1b[33m WARN\x1b[0m victory_gpu::device: a pipeline creation method that is not implemented\n\
        VIctory: first-chance exception 0x80000003 at victory-image-b99f876b74d4a77f.dll+0x1dac734\n\
        \x20 rax 0x0 rbx 0x908a70860\n\
        \x20 #0  victory-image-b99f876b74d4a77f.dll+0x1dac734\n\
        VIctory: unhandled exception 0x80000003 at victory-image-b99f876b74d4a77f.dll+0x1dac734\n\
        \x20 rax 0x0 rbx 0x908a70860\n\
        \x20 #0  victory-image-b99f876b74d4a77f.dll+0x1dac734\n\
        \x20 #1  victory-image-b99f876b74d4a77f.dll+0x5b97b29\n";

    #[test]
    fn an_unknown_stop_is_named_with_its_place() {
        let r = explain(RADEON, Some(-2147483645), true);
        assert!(r.short.starts_with("parada interna do jogo"), "{}", r.short);
        assert!(r.detail.contains("unhandled exception 0x80000003 at victory-image-b99f876b74d4a77f.dll+0x1dac734"));
        assert!(r.detail.contains("#1  victory-image-b99f876b74d4a77f.dll+0x5b97b29"));
        assert!(r.detail.contains("-2147483645 (0x80000003)"));
        assert!(!r.detail.contains('\x1b'));
        // Pipeline warnings are not the cause.
        assert!(!r.detail.contains("pipeline"));
    }

    #[test]
    fn known_stops_say_what_to_do() {
        let log = "VIctory: unhandled exception 0x80000003 at victory-image-b99f876b74d4a77f.dll+0x2219650\n  #0  x\n";
        let r = explain(log, Some(-2147483645), true);
        assert!(r.short.contains("reservar memória"), "{}", r.short);
        assert!(r.detail.contains("Qualidade"));
        let r = explain(&log.replace("2219650", "2237178"), Some(-2147483645), false);
        assert!(r.short.contains("appdata0.rpf"), "{}", r.short);
        let amd = "VIctory: unhandled exception 0xc0000005 at amdxc64.dll+0x861b9f (read 0x8: nothing mapped there)\n  #0  amdxc64.dll+0x861b9f\n";
        let r = explain(amd, Some(-1073741819), true);
        assert!(r.short.contains("driver AMD"), "{}", r.short);
        assert!(r.detail.contains("amdxc64.dll+0x861b9f"));
    }

    #[test]
    fn engine_errors_panics_and_memory_are_read() {
        let log = "Error: the game folder has no shaders.rpf\n\nCaused by:\n    0: open failed\n";
        let r = explain(log, Some(1), true);
        assert_eq!(r.short, "the game folder has no shaders.rpf");
        assert!(r.detail.contains("  0: open failed"));

        let log = "thread 'main' panicked at crates/x.rs:10:5:\nindex out of bounds\n";
        let r = explain(log, Some(101), false);
        assert!(r.short.contains("index out of bounds"), "{}", r.short);

        let log = "WARN victory_gpu::residency: host memory pressure: limiting texture prefetch pid=1 available_commit_mib=2047 x=1\n\
                   Error: MapViewOfFile failed (os error 1455)\n";
        let r = explain(log, Some(1), true);
        assert!(r.detail.contains("2.0 GB"), "{}", r.detail);
        assert!(r.detail.contains("arquivo de paginação"));
    }

    #[test]
    fn a_bare_exit_code_is_named() {
        let r = explain("", Some(-1073741819), true);
        assert!(r.short.contains("acesso inválido") && r.short.contains("0xc0000005"), "{}", r.short);
        let r = explain("", None, false);
        assert_eq!(r.short, "the process was ended");
    }

    #[test]
    fn a_pipeline_that_stops_the_driver_is_left_out() {
        let log = "VIctory: first-chance exception 0xc0000005 at amdxc64.dll+0x861b9f (read 0x8: nothing mapped there)\n\
                   \x20 while making the host pipeline of [compute 1111111111111111]\n\
                   VIctory: unhandled exception 0xc0000005 at amdxc64.dll+0x861b9f (read 0x8: nothing mapped there)\n\
                   \x20 while making the host pipeline of [vertex 240CB516C1AF0BC3, pixel 0123456789abcdef]\n\
                   \x20 rax 0x0\n\
                   \x20 #0  amdxc64.dll+0x861b9f\n";
        assert_eq!(left_out_programs(log), ["240cb516c1af0bc3", "0123456789abcdef"]);
        let r = explain(log, Some(-1073741819), true);
        assert_eq!(r.leave_out, ["240cb516c1af0bc3", "0123456789abcdef"]);
        assert!(r.short.contains("compilador de shaders"), "{}", r.short);
        assert!(r.detail.contains("deixar de fora") && r.detail.contains("leave-out.txt"), "{}", r.detail);
        assert!(left_out_programs("VIctory: unhandled exception 0xc0000005 at x.dll+0x1\n  #0 x\n").is_empty());
    }

    #[test]
    fn memory_running_out_is_named_over_the_driver_crash() {
        let log = "WARN victory_gpu::residency: host memory pressure: limiting pid=1 available_commit_mib=17240 x=1\n\
                   WARN victory_gpu::host: no upload buffer error=Out of memory. (0x8007000E) size=262144\n\
                   WARN victory_gpu::host: no upload buffer error=Out of memory. (0x8007000E) size=4194304\n\
                   VIctory: unhandled exception 0xc0000005 at nvwgf2umx.dll+0x34a44a (read 0xc: nothing mapped there)\n\
                   \x20 #0  nvwgf2umx.dll+0x34a44a\n";
        let r = explain(log, Some(-1073741819), false);
        assert!(r.short.contains("memory ran out"), "{}", r.short);
        assert!(r.detail.contains("8 GB") && r.detail.contains("2x"), "{}", r.detail);
        // 17 GB of free virtual memory is no note.
        assert!(!r.detail.contains("close to its limit"), "{}", r.detail);
        let r = explain(&log.replace("0x8007000E", "0x0"), None, false);
        assert!(r.short.contains("NVIDIA"), "{}", r.short);
    }
}

#[cfg(test)]
mod log_file_tests {
    /// With `VICTORY_CRASH_LOG=<game.log>` (and `VICTORY_CRASH_CODE`): the
    /// notice for a real log, printed (`--ignored --nocapture`).
    #[test]
    #[ignore]
    fn explains_a_log_file() {
        let path = std::env::var("VICTORY_CRASH_LOG").expect("VICTORY_CRASH_LOG");
        let log = String::from_utf8_lossy(&std::fs::read(path).unwrap()).into_owned();
        let code = std::env::var("VICTORY_CRASH_CODE").ok().and_then(|c| c.parse().ok());
        let report = super::explain(&log, code, true);
        println!("STATUS: {}\n---\n{}", report.short, report.detail);
    }
}
