//! rg52-info: mostra as informacoes do sistema na tela do aparelho (so leitura, nao
//! precisa de root). Atualiza a cada segundo; B/Esc/Start sai.

use std::{
    env,
    path::PathBuf,
    process,
    time::{Duration, Instant},
};

use rg52info::{
    devices,
    gfx::{Color, Fonts, Pixmap},
    sysinfo::{local_time, Sys},
    ui::{page_count, parse_accent, render, Screen, ACCENT},
};
use sdl2::{
    controller::{Button, GameController},
    event::{Event, WindowEvent},
    keyboard::Keycode,
    pixels::PixelFormatEnum,
};

const HELP: &str = "\
rg52-info - informacoes do sistema na tela (RG52 Mini / dArkOS)

USO: rg52-info [opcoes]
  --device ID       auto (padrao), rg52mini, rg43, r36s, rgb10, a10mini, rk3566 ou generic
  --fullscreen      tela cheia (use no aparelho)
  --no-gamepad      ignora o gamepad do SDL (use com o gptokeyb)
  --interval S      segundos entre atualizacoes (padrao 1)
  --accent COR      amber (padrao), teal, violet, coral ou #RRGGBB
  --root PATH       le /proc e /sys de outra raiz (testes)
  --screenshot PPM  (depuracao) desenha um quadro sem janela, salva em PPM e sai
  --size LxA        tamanho do --screenshot (padrao 1280x720)
  --page N          pagina inicial nas telas compactas (padrao 1)
  -h, --help        esta ajuda
";

struct Args {
    device: Option<String>,
    size: (usize, usize),
    page: usize,
    fullscreen: bool,
    no_gamepad: bool,
    interval: f32,
    accent: Color,
    root: PathBuf,
    screenshot: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args {
        device: None,
        size: (1280, 720),
        page: 0,
        fullscreen: false,
        no_gamepad: false,
        interval: 1.0,
        accent: ACCENT,
        root: PathBuf::from("/"),
        screenshot: None,
    };
    let mut it = env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut val = |n: &str| it.next().ok_or_else(|| format!("{n} precisa de um valor"));
        match arg.as_str() {
            "--device" => a.device = Some(val("--device")?),
            "--size" => {
                let s = val("--size")?;
                a.size = s
                    .split_once('x')
                    .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
                    .filter(|&(w, h): &(usize, usize)| w > 0 && h > 0)
                    .ok_or_else(|| format!("--size invalido: {s:?} (use LxA, ex.: 640x480)"))?
            }
            "--page" => {
                a.page = val("--page")?
                    .parse::<usize>()
                    .ok()
                    .filter(|p| *p > 0)
                    .ok_or("--page invalido")?
                    - 1
            }
            "--fullscreen" => a.fullscreen = true,
            "--no-gamepad" => a.no_gamepad = true,
            "--interval" => {
                a.interval = val("--interval")?
                    .parse::<f32>()
                    .ok()
                    .filter(|v| *v >= 0.1)
                    .ok_or("--interval invalido (minimo 0.1)")?
            }
            "--accent" => a.accent = parse_accent(&val("--accent")?)?,
            "--root" => a.root = PathBuf::from(val("--root")?),
            "--screenshot" => a.screenshot = Some(PathBuf::from(val("--screenshot")?)),
            "-h" | "--help" => {
                print!("{HELP}");
                process::exit(0);
            }
            o => return Err(format!("opcao desconhecida: {o}\n\n{HELP}")),
        }
    }
    Ok(a)
}

fn is_exit_key(k: Keycode) -> bool {
    matches!(
        k,
        Keycode::Escape | Keycode::Backspace | Keycode::Q | Keycode::Return | Keycode::KpEnter
    )
}

/// Proxima pagina (ou anterior), dando a volta.
fn turn(page: usize, pages: usize, next: bool) -> usize {
    let page = page % pages;
    if next {
        (page + 1) % pages
    } else {
        (page + pages - 1) % pages
    }
}

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            process::exit(2);
        }
    };
    if let Err(e) = run(args) {
        eprintln!("erro: {e}");
        process::exit(1);
    }
}

fn run(args: Args) -> Result<(), String> {
    let sys = Sys::new(&args.root);
    let fonts = Fonts::new();
    // Compativel com qualquer aparelho (so le /proc e /sys): o perfil da o nome e a tela.
    let detected = devices::resolve(args.device.as_deref())?;
    let title = detected.profile.title;
    let mut page = args.page;

    if let Some(path) = &args.screenshot {
        let mut pm = Pixmap::new(args.size.0, args.size.1);
        let screen = Screen { title, page };
        render(
            &mut pm,
            &fonts,
            &sys.collect(),
            &local_time(),
            args.accent,
            &screen,
        );
        return pm.write_ppm(path).map_err(|e| e.to_string());
    }

    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let gcs = sdl.game_controller()?;
    let mut pads: Vec<GameController> = Vec::new();
    if !args.no_gamepad {
        for i in 0..gcs.num_joysticks()? {
            if gcs.is_game_controller(i) {
                if let Ok(p) = gcs.open(i) {
                    pads.push(p);
                }
            }
        }
    }
    let mut wb = video.window(&format!("{title} INFO"), 960, 540);
    wb.position_centered().resizable();
    if args.fullscreen {
        wb.fullscreen_desktop();
    }
    let window = wb.build().map_err(|e| e.to_string())?;
    let mut canvas = window
        .into_canvas()
        .present_vsync()
        .build()
        .map_err(|e| e.to_string())?;
    if args.fullscreen {
        sdl.mouse().show_cursor(false);
    }
    let tc = canvas.texture_creator();
    let mut pump = sdl.event_pump()?;
    let (mut w, mut h) = canvas.output_size()?;
    let mut pm = Pixmap::new(w as usize, h as usize);
    let mut tex = tc
        .create_texture_streaming(PixelFormatEnum::ARGB8888, w, h)
        .map_err(|e| e.to_string())?;

    let every = Duration::from_secs_f32(args.interval);
    let mut last = Instant::now() - every;
    let mut force = true;
    let mut quit = false;
    while !quit {
        for ev in pump.poll_iter() {
            match ev {
                Event::Quit { .. } => quit = true,
                Event::KeyDown {
                    keycode: Some(k), ..
                } if is_exit_key(k) => quit = true,
                Event::KeyDown {
                    keycode: Some(Keycode::Left | Keycode::PageUp),
                    ..
                } => {
                    page = turn(page, page_count(w as usize, h as usize), false);
                    force = true;
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Right | Keycode::PageDown),
                    ..
                } => {
                    page = turn(page, page_count(w as usize, h as usize), true);
                    force = true;
                }
                Event::ControllerButtonDown {
                    button:
                        button @ (Button::DPadLeft
                        | Button::DPadRight
                        | Button::LeftShoulder
                        | Button::RightShoulder),
                    ..
                } if !args.no_gamepad => {
                    let next = matches!(button, Button::DPadRight | Button::RightShoulder);
                    page = turn(page, page_count(w as usize, h as usize), next);
                    force = true;
                }
                Event::MouseButtonDown { .. } => {}
                Event::ControllerButtonDown { button, .. }
                    if !args.no_gamepad
                        && matches!(button, Button::B | Button::Start | Button::A) =>
                {
                    quit = true
                }
                Event::ControllerDeviceAdded { which, .. } if !args.no_gamepad => {
                    if let Ok(p) = gcs.open(which) {
                        pads.push(p);
                    }
                }
                Event::ControllerDeviceRemoved { which, .. } => {
                    pads.retain(|p| p.instance_id() != which)
                }
                Event::Window {
                    win_event:
                        WindowEvent::SizeChanged(..)
                        | WindowEvent::Exposed
                        | WindowEvent::Restored
                        | WindowEvent::Shown,
                    ..
                } => force = true,
                _ => {}
            }
        }
        let (nw, nh) = canvas.output_size()?;
        if (nw, nh) != (w, h) {
            (w, h) = (nw, nh);
            pm = Pixmap::new(w as usize, h as usize);
            tex = tc
                .create_texture_streaming(PixelFormatEnum::ARGB8888, w, h)
                .map_err(|e| e.to_string())?;
            force = true;
        }
        if force || last.elapsed() >= every {
            last = Instant::now();
            let screen = Screen { title, page };
            render(
                &mut pm,
                &fonts,
                &sys.collect(),
                &local_time(),
                args.accent,
                &screen,
            );
            tex.update(None, pm.data(), pm.pitch())
                .map_err(|e| e.to_string())?;
            canvas.copy(&tex, None, None)?;
            canvas.present();
            force = false;
        } else {
            std::thread::sleep(Duration::from_millis(30));
        }
    }
    Ok(())
}
