//! Tela "DEVICE INCOMPATIBLE": aparece no lugar do app quando o aparelho nao tem o recurso.

use super::Detected;
use crate::gfx::{Color, Face, Fonts, Pixmap};

const BG: Color = Color::hex(0x0B0D12);
const CARD: Color = Color::hex(0x12161F);
const LINE: Color = Color::hex(0x1D2332);
const TEXT: Color = Color::hex(0xE8ECF4);
const MUTED: Color = Color::hex(0x8E98AD);
pub const BAD: Color = Color::hex(0xFF6B6B);
const KEY_BG: Color = Color::hex(0x1B2130);
const KEY_EDGE: Color = Color::hex(0x2A3144);

const DESIGN_W: f32 = 640.0;
const DESIGN_H: f32 = 360.0;

/// Desenha a tela em qualquer resolucao (design 640x360, centralizado e escalado).
pub fn render(
    pm: &mut Pixmap,
    fonts: &Fonts,
    app: &str,
    det: &Detected,
    reason: &str,
    accent: Color,
) {
    let (w, h) = (pm.width() as f32, pm.height() as f32);
    let k = (w / DESIGN_W).min(h / DESIGN_H);
    let (ox, oy) = ((w - DESIGN_W * k) / 2.0, (h - DESIGN_H * k) / 2.0);
    pm.clear(BG);
    let (cx, cw) = (ox + 320.0 * k, 488.0 * k);
    let (x, y, rw, rh) = (ox + 60.0 * k, oy + 50.0 * k, 520.0 * k, 260.0 * k);
    pm.fill_rrect(x, y, rw, rh, 20.0 * k, CARD);
    pm.stroke_rrect(x, y, rw, rh, 20.0 * k, k, LINE);

    let center = |pm: &mut Pixmap, face, size: f32, c, dy: f32, s: &str| {
        let s = fonts.fit(face, size * k, 0.0, s, cw);
        let tw = fonts.width(face, size * k, 0.0, &s);
        let (a, d) = fonts.metrics(face);
        let base = oy + dy * k + (a + d) * size * k / 2.0;
        pm.text(fonts, face, size * k, 0.0, c, cx - tw / 2.0, base, &s);
    };
    pm.stroke_circle(cx, oy + 92.0 * k, 17.0 * k, 3.0 * k, BAD);
    center(pm, Face::Sans800, 22.0, BAD, 92.0, "!");
    center(pm, Face::Sans800, 28.0, TEXT, 144.0, "DEVICE INCOMPATIBLE");
    center(pm, Face::Sans700, 15.0, accent, 178.0, app);
    center(pm, Face::Mono700, 14.0, TEXT, 212.0, det.label());
    center(pm, Face::Sans600, 14.0, MUTED, 238.0, reason);

    // tecla de saida
    let (kw, kh, ky) = (28.0 * k, 24.0 * k, oy + 270.0 * k);
    let label_w = fonts.width(Face::Sans600, 13.0 * k, 0.0, "Exit");
    let kx = cx - (kw + 8.0 * k + label_w) / 2.0;
    pm.fill_rrect(kx, ky, kw, kh, 7.0 * k, KEY_BG);
    pm.stroke_rrect(kx, ky, kw, kh, 7.0 * k, k, KEY_EDGE);
    let bw = fonts.width(Face::Mono700, 11.0 * k, 0.0, "B");
    let mid = |face: Face, size: f32| {
        let (a, d) = fonts.metrics(face);
        ky + kh / 2.0 + (a + d) * size * k / 2.0
    };
    pm.text(
        fonts,
        Face::Mono700,
        11.0 * k,
        0.0,
        TEXT,
        kx + (kw - bw) / 2.0,
        mid(Face::Mono700, 11.0),
        "B",
    );
    pm.text(
        fonts,
        Face::Sans600,
        13.0 * k,
        0.0,
        MUTED,
        kx + kw + 8.0 * k,
        mid(Face::Sans600, 13.0),
        "Exit",
    );
}

/// Abre a janela com a tela e espera qualquer botao para sair.
#[cfg(feature = "ui")]
pub fn show(
    app: &str,
    det: &Detected,
    reason: &str,
    accent: Color,
    fullscreen: bool,
    no_gamepad: bool,
) -> Result<(), String> {
    use sdl2::{
        event::{Event, WindowEvent},
        pixels::PixelFormatEnum,
    };
    use std::time::{Duration, Instant};

    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let gcs = sdl.game_controller()?;
    let mut pads = Vec::new();
    if !no_gamepad {
        for i in 0..gcs.num_joysticks()? {
            if gcs.is_game_controller(i) {
                if let Ok(p) = gcs.open(i) {
                    pads.push(p);
                }
            }
        }
    }
    let mut wb = video.window(app, 960, 540);
    wb.position_centered().resizable();
    if fullscreen {
        wb.fullscreen_desktop();
    }
    let window = wb.build().map_err(|e| e.to_string())?;
    let mut canvas = window
        .into_canvas()
        .present_vsync()
        .build()
        .map_err(|e| e.to_string())?;
    if fullscreen {
        sdl.mouse().show_cursor(false);
    }
    let tc = canvas.texture_creator();
    let mut pump = sdl.event_pump()?;
    let fonts = Fonts::new();
    let (mut w, mut h) = (0, 0);
    let mut pm = Pixmap::new(1, 1);
    let mut tex = None;
    let mut force = true;
    // ignora o botao que abriu o app e ainda pode estar chegando
    let opened = Instant::now();
    loop {
        for ev in pump.poll_iter() {
            let ready = opened.elapsed() > Duration::from_millis(400);
            match ev {
                Event::Quit { .. } => return Ok(()),
                Event::KeyDown { repeat: false, .. } | Event::ControllerButtonDown { .. }
                    if ready =>
                {
                    return Ok(())
                }
                Event::Window {
                    win_event: WindowEvent::SizeChanged(..) | WindowEvent::Exposed,
                    ..
                } => force = true,
                _ => {}
            }
        }
        let (nw, nh) = canvas.output_size()?;
        if (nw, nh) != (w, h) {
            (w, h) = (nw, nh);
            pm = Pixmap::new(w as usize, h as usize);
            render(&mut pm, &fonts, app, det, reason, accent);
            tex = Some(
                tc.create_texture_streaming(PixelFormatEnum::ARGB8888, w, h)
                    .map_err(|e| e.to_string())?,
            );
            force = true;
        }
        if let (true, Some(t)) = (force, tex.as_mut()) {
            t.update(None, pm.data(), pm.pitch())
                .map_err(|e| e.to_string())?;
            canvas.copy(t, None, None)?;
            canvas.present();
            force = false;
        }
        std::thread::sleep(Duration::from_millis(16));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::identify;

    #[test]
    fn desenha_em_todas_as_telas() {
        let det = identify(Some("RG351MP"), None, None);
        let fonts = Fonts::new();
        for (w, h) in [
            (480, 320),
            (640, 480),
            (720, 720),
            (1024, 768),
            (1280, 720),
            (1, 1),
        ] {
            let mut pm = Pixmap::new(w, h);
            render(&mut pm, &fonts, "LED RGB", &det, "No compatible LED.", BAD);
            if w > 1 {
                let hit = pm.data().chunks_exact(4).any(|p| p[2] > 240 && p[1] < 130);
                assert!(hit, "{w}x{h}: titulo/icone vermelho nao apareceu");
            }
        }
    }
}
