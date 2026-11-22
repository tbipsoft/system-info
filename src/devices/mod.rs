//! Especificacoes dos aparelhos e o que cada app suporta em cada um.
//!
//! Esta pasta e identica em led-rgb, remote-services e system-info (como o `gfx.rs`):
//! ao mudar um arquivo aqui, copie para os outros dois apps.
//! Para adicionar um modelo: inclua um `Profile` em `profiles.rs` e registre-o em `ALL`.
//! A resolucao nao fica aqui de proposito: a tela usa o tamanho real informado pelo SDL.

mod detect;
mod profiles;
pub mod screen;

pub use detect::{identify, resolve, Detected, Source};
pub use profiles::{ALL, GENERIC};

#[derive(Debug, PartialEq, Eq)]
pub struct Profile {
    /// Identificador aceito em `--device`.
    pub id: &'static str,
    /// Nome curto do cabecalho.
    pub title: &'static str,
    /// Nome completo.
    pub name: &'static str,
    /// Valores de `/home/ark/.config/.DEVICE` (ou hostname) que indicam este perfil.
    pub markers: &'static [&'static str],
    /// MCU de LED na UART1 (`/dev/ttyS1`), o hardware que o led-rgb controla.
    pub rgb_led: bool,
}

/// Recursos que dependem do aparelho.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feature {
    RgbLed,
    RemoteServices,
    SystemInfo,
}

/// Maiusculas, sem espacos, `-`, `_` nem NUL: `rg-52 mini\n` e `RG52MINI` sao iguais.
pub(crate) fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// Procura um perfil pelo `id` ou por um dos marcadores.
pub fn find(name: &str) -> Option<&'static Profile> {
    let n = normalize(name);
    if n.is_empty() {
        return None;
    }
    ALL.into_iter()
        .find(|p| normalize(p.id) == n || p.markers.iter().any(|m| normalize(m) == n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfis_nao_repetem_marcadores() {
        let mut seen = std::collections::HashSet::new();
        for p in ALL {
            assert!(seen.insert(normalize(p.id)), "id repetido: {}", p.id);
            for m in p.markers {
                assert!(
                    normalize(m) == normalize(p.id) || seen.insert(normalize(m)),
                    "marcador repetido: {m}"
                );
            }
        }
    }

    #[test]
    fn so_a_familia_rk3562_tem_o_led_da_uart() {
        let with_led: Vec<_> = ALL.iter().filter(|p| p.rgb_led).map(|p| p.id).collect();
        assert_eq!(with_led, ["rg52mini", "rg43"]);
    }
}
