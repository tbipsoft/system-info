//! Cabecalho comum dos apps: logo Tbip a esquerda e, no centro, aparelho + app com a
//! versao logo abaixo. Arquivo identico em todos os apps (como o `gfx.rs`).

use crate::gfx::{Color, Face, Fonts, Pixmap};

/// `docs/tbip-letras-branca.svg` rasterizado (RGBA nao pre-multiplicado).
const LOGO: &[u8] = include_bytes!("../assets/logo_tbip_248x87.rgba");
const LOGO_SRC: (usize, usize) = (248, 87);

const TEXT: Color = Color::hex(0xE8ECF4);
const MUTED: Color = Color::hex(0x8E98AD);
const VERSION: &str = concat!("v", env!("CARGO_PKG_VERSION"));

pub struct Header<'a> {
    /// Margem esquerda do logo, em coordenadas do design.
    pub left: f32,
    /// Centro horizontal do design.
    pub center: f32,
    /// Centro vertical do cabecalho.
    pub cy: f32,
    pub compact: bool,
    pub device: &'a str,
    pub app: &'a str,
}

/// Desenha em `pm`; `k`, `ox` e `oy` convertem do design para pixels.
pub fn draw(pm: &mut Pixmap, fonts: &Fonts, k: f32, ox: f32, oy: f32, h: &Header) {
    let (logo_h, title, version, title_dy, version_dy) = if h.compact {
        (24.0, 15.0, 10.0, -7.0, 9.0)
    } else {
        (34.0, 20.0, 12.0, -9.0, 13.0)
    };
    logo(
        pm,
        ox + h.left * k,
        oy + (h.cy - logo_h / 2.0) * k,
        logo_h * k,
    );

    let mid = |face: Face, size: f32, cy: f32| {
        let (a, d) = fonts.metrics(face);
        oy + (cy + (a + d) * size / 2.0) * k
    };
    let gap = 6.0 * k;
    let w1 = fonts.width(Face::Sans800, title * k, 0.0, h.device);
    let w2 = fonts.width(Face::Sans500, title * k, 0.0, h.app);
    let x = ox + h.center * k - (w1 + gap + w2) / 2.0;
    let y = h.cy + title_dy;
    pm.text(
        fonts,
        Face::Sans800,
        title * k,
        0.0,
        TEXT,
        x,
        mid(Face::Sans800, title, y),
        h.device,
    );
    pm.text(
        fonts,
        Face::Sans500,
        title * k,
        0.0,
        MUTED,
        x + w1 + gap,
        mid(Face::Sans500, title, y),
        h.app,
    );
    let vw = fonts.width(Face::Mono500, version * k, 0.0, VERSION);
    pm.text(
        fonts,
        Face::Mono500,
        version * k,
        0.0,
        MUTED,
        ox + h.center * k - vw / 2.0,
        mid(Face::Mono500, version, h.cy + version_dy),
        VERSION,
    );
}

/// Cola o logo com altura `px_h`; a versao reamostrada fica em cache.
fn logo(pm: &mut Pixmap, x: f32, y: f32, px_h: f32) {
    thread_local! {
        static CACHE: std::cell::RefCell<Option<(usize, usize, Pixmap)>> =
            const { std::cell::RefCell::new(None) };
    }
    let dh = px_h.round() as usize;
    let dw = (px_h * LOGO_SRC.0 as f32 / LOGO_SRC.1 as f32).round() as usize;
    if dw == 0 || dh == 0 {
        return;
    }
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if !matches!(&*c, Some((w, h, _)) if *w == dw && *h == dh) {
            let scaled = Pixmap::from_rgba_scaled(LOGO, LOGO_SRC.0, LOGO_SRC.1, dw, dh);
            *c = Some((dw, dh, scaled));
        }
        if let Some((_, _, scaled)) = &*c {
            pm.blit(scaled, x.round() as i32, y.round() as i32, 1.0);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_tem_o_tamanho_certo_e_as_duas_cores() {
        assert_eq!(LOGO.len(), LOGO_SRC.0 * LOGO_SRC.1 * 4);
        let solid = |r: u8, g: u8, b: u8| {
            LOGO.chunks_exact(4)
                .any(|p| p[0] == r && p[1] == g && p[2] == b && p[3] == 255)
        };
        assert!(solid(0xFF, 0x66, 0x00), "laranja");
        assert!(solid(0xFE, 0xFE, 0xFE), "branco");
    }

    #[test]
    fn desenha_logo_a_esquerda_e_texto_no_centro() {
        let mut pm = Pixmap::new(640, 60);
        pm.clear(Color::rgb(0, 0, 0));
        let h = Header {
            left: 10.0,
            center: 320.0,
            cy: 20.0,
            compact: true,
            device: "R36S",
            app: "INFO",
        };
        draw(&mut pm, &Fonts::new(), 1.0, 0.0, 0.0, &h);
        let inked = |x0: usize, x1: usize| {
            (x0..x1).any(|x| (0..60).any(|y| pm.get(x, y).unwrap()[0] > 120))
        };
        assert!(inked(10, 78), "logo");
        assert!(inked(280, 360), "titulo");
        assert!(!inked(100, 250), "espaco entre logo e titulo");
    }
}
