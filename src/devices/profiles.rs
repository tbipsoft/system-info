//! Aparelhos conhecidos. Marcadores vindos dos scripts de build do dArkOS
//! (christianhaitian/dArkOS) e do fork RK3562 (bmdhacks/dArkOS_rg52mini).

use super::Profile;

/// RK3562, 1280x720. LEDs dos analogicos na MCU da UART1.
pub const RG52MINI: Profile = Profile {
    id: "rg52mini",
    title: "RG52",
    name: "RG52 Mini (RK3562)",
    markers: &["RG52MINI", "RG52"],
    rgb_led: true,
};

/// RK3562, 1024x768. A imagem do RG43 V Pro tambem grava `RG43H`.
pub const RG43: Profile = Profile {
    id: "rg43",
    title: "RG43",
    name: "RG43 Pro (RK3562)",
    markers: &["RG43H", "RG43V", "RG43"],
    rgb_led: true,
};

pub const R36S: Profile = Profile {
    id: "r36s",
    title: "R36S",
    name: "R36S / RG351MP (RK3326)",
    markers: &["RG351MP", "RG351", "R36S"],
    rgb_led: false,
};

pub const RGB10: Profile = Profile {
    id: "rgb10",
    title: "RGB10",
    name: "RGB10 (RK3326)",
    markers: &["RGB10"],
    rgb_led: false,
};

pub const A10MINI: Profile = Profile {
    id: "a10mini",
    title: "A10 MINI",
    name: "A10 Mini / A10 Mini V4 (RK3326)",
    markers: &["A10MINI", "A10MINIV4"],
    rgb_led: false,
};

/// O Miniloong tem LEDs, mas em outro chip (aw20036) e com outro protocolo.
pub const RK3566: Profile = Profile {
    id: "rk3566",
    title: "RK3566",
    name: "Familia RK3566",
    markers: &[
        "RK3566",
        "RG353M",
        "RG353V",
        "RG503",
        "RK2023",
        "RGB30",
        "RGB20PRO",
        "MINILOONG",
    ],
    rgb_led: false,
};

/// Aparelho nao identificado.
pub const GENERIC: Profile = Profile {
    id: "generic",
    title: "DARKOS",
    name: "Unknown device",
    markers: &[],
    rgb_led: false,
};

pub const ALL: [&Profile; 7] = [&RG52MINI, &RG43, &R36S, &RGB10, &A10MINI, &RK3566, &GENERIC];
