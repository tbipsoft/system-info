//! Descobre em qual aparelho o app esta rodando e o que ele suporta.
//!
//! Ordem: `--device`, `/home/ark/.config/.DEVICE` (gravado pelo build do dArkOS), hostname
//! e familia do chip no device tree. Sem nenhuma pista, usa o perfil generico.

use std::fs;

use super::{find, normalize, Feature, Profile, ALL, GENERIC};

const MARKER_FILE: &str = "/home/ark/.config/.DEVICE";
const HOSTNAME_FILE: &str = "/etc/hostname";
const COMPATIBLE_FILE: &str = "/proc/device-tree/compatible";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Manual,
    MarkerFile,
    Hostname,
    DeviceTree,
    Fallback,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Detected {
    pub profile: &'static Profile,
    pub source: Source,
    /// Conteudo de `.DEVICE`, se existir: indica que o sistema e um dArkOS.
    pub marker: Option<String>,
}

impl Detected {
    pub fn supports(&self, f: Feature) -> bool {
        match f {
            Feature::RgbLed => self.profile.rgb_led,
            // depende dos servicos da imagem: qualquer dArkOS, mesmo de modelo desconhecido
            Feature::RemoteServices => self.marker.is_some() || *self.profile != GENERIC,
            Feature::SystemInfo => true,
        }
    }

    /// Nome para mostrar; em aparelho desconhecido, o que o `.DEVICE` informa.
    pub fn label(&self) -> &str {
        match &self.marker {
            Some(m) if *self.profile == GENERIC => m,
            _ => self.profile.name,
        }
    }
}

/// Decisao pura, testavel sem o aparelho.
pub fn identify(
    marker: Option<&str>,
    hostname: Option<&str>,
    compatible: Option<&str>,
) -> Detected {
    let marker = marker.map(str::trim).filter(|m| !m.is_empty());
    let hit = |profile, source| Detected {
        profile,
        source,
        marker: marker.map(str::to_string),
    };
    if let Some(p) = marker.and_then(find) {
        return hit(p, Source::MarkerFile);
    }
    if let Some(p) = hostname.and_then(find) {
        return hit(p, Source::Hostname);
    }
    // rk3326 nao decide: R36S, RGB10, A10 Mini e G350 usam o mesmo chip.
    let chip = compatible.map(normalize).unwrap_or_default();
    for (name, id) in [("RK3562", "rg52mini"), ("RK3566", "rk3566")] {
        if chip.contains(name) {
            if let Some(p) = find(id) {
                return hit(p, Source::DeviceTree);
            }
        }
    }
    hit(&GENERIC, Source::Fallback)
}

fn detect() -> Detected {
    let read = |path| fs::read_to_string(path).ok();
    // o device tree tem NULs e pode nao ser UTF-8 valido
    let compatible = fs::read(COMPATIBLE_FILE)
        .ok()
        .map(|b| String::from_utf8_lossy(&b).replace('\0', " "));
    identify(
        read(MARKER_FILE).as_deref(),
        read(HOSTNAME_FILE).as_deref(),
        compatible.as_deref(),
    )
}

/// `None` ou `auto` detectam; qualquer outro valor escolhe o perfil manualmente.
pub fn resolve(requested: Option<&str>) -> Result<Detected, String> {
    match requested {
        None => Ok(detect()),
        Some(r) if r.eq_ignore_ascii_case("auto") => Ok(detect()),
        Some(r) => find(r)
            .map(|profile| Detected {
                profile,
                source: Source::Manual,
                marker: None,
            })
            .ok_or_else(|| {
                let ids: Vec<_> = ALL.iter().map(|p| p.id).collect();
                format!("--device invalido: {r:?} (use auto, {})", ids.join(", "))
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(m: Option<&str>, h: Option<&str>, c: Option<&str>) -> (&'static str, Source) {
        let d = identify(m, h, c);
        (d.profile.id, d.source)
    }

    #[test]
    fn marcadores_gravados_pelo_darkos() {
        for (marker, want) in [
            ("RG52MINI\n", "rg52mini"),
            ("RG43H\n", "rg43"),
            ("RG351MP\n", "r36s"),
            ("RGB10\n", "rgb10"),
            ("A10MINIV4\n", "a10mini"),
            ("RG503\n", "rk3566"),
            ("MINILOONG\n", "rk3566"),
        ] {
            assert_eq!(id(Some(marker), None, None), (want, Source::MarkerFile));
        }
    }

    #[test]
    fn ordem_de_prioridade_e_chip() {
        assert_eq!(
            id(Some("RGB10"), Some("rg43h"), Some("rockchip,rk3562")).0,
            "rgb10"
        );
        assert_eq!(
            id(Some("XYZ"), Some("rg52mini\n"), None),
            ("rg52mini", Source::Hostname)
        );
        assert_eq!(
            id(None, Some("debian"), Some("rockchip,rk3562\0")),
            ("rg52mini", Source::DeviceTree)
        );
        assert_eq!(id(None, None, Some("rockchip,rk3566\0")).0, "rk3566");
        assert_eq!(
            id(None, Some("debian"), Some("rockchip,rk3326\0")),
            ("generic", Source::Fallback)
        );
    }

    #[test]
    fn compatibilidade_por_recurso() {
        let rg52 = identify(Some("RG52MINI"), None, None);
        let r36 = identify(Some("RG351MP"), None, None);
        let novo = identify(Some("RG56PRO\n"), None, None);
        let pc = identify(None, Some("meu-pc"), None);
        assert!(rg52.supports(Feature::RgbLed));
        for d in [&r36, &novo, &pc] {
            assert!(!d.supports(Feature::RgbLed), "{}", d.label());
        }
        assert!(r36.supports(Feature::RemoteServices));
        assert!(
            novo.supports(Feature::RemoteServices),
            "dArkOS desconhecido"
        );
        assert!(!pc.supports(Feature::RemoteServices), "fora do dArkOS");
        for d in [&rg52, &r36, &novo, &pc] {
            assert!(d.supports(Feature::SystemInfo));
        }
        assert_eq!(novo.label(), "RG56PRO");
        assert_eq!(rg52.label(), "RG52 Mini (RK3562)");
    }

    #[test]
    fn escolha_manual() {
        let d = resolve(Some("rg-52 mini")).unwrap();
        assert_eq!((d.profile.id, d.source), ("rg52mini", Source::Manual));
        assert!(resolve(Some("gameboy")).unwrap_err().contains("r36s"));
    }
}
