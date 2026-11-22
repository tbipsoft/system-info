//! Coleta das informacoes (as mesmas do `System Info.sh`), lendo /proc e /sys direto,
//! sem executar comandos. Tudo parte de uma raiz (`Sys::root`), entao os testes usam uma
//! arvore falsa em /tmp. Qualquer leitura que falhe vira `None`/0: o aparelho muda de
//! um modelo para outro e a tela nunca deve quebrar por falta de um arquivo.

use std::{
    ffi::CString,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Battery {
    /// 0..=100
    pub capacity: Option<u8>,
    pub status: String,
    /// Volts
    pub voltage: Option<f32>,
    /// Amperes (negativo = descarregando, conforme a firmware informa)
    pub current: Option<f32>,
    /// Graus C
    pub temp: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Disk {
    pub path: String,
    /// Dispositivo montado em `path` (/proc/mounts), `None` se nao for um ponto de montagem
    pub fs: Option<String>,
    pub total: u64,
    pub used: u64,
}

impl Disk {
    pub fn free(&self) -> u64 {
        self.total.saturating_sub(self.used)
    }
    pub fn pct(&self) -> f32 {
        pct(self.used, self.total)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Info {
    pub host: String,
    pub users: Vec<String>,
    pub uptime_s: u64,
    pub load: [f32; 3],
    pub cores: usize,
    pub cpu_model: String,
    pub cpu_type: String,
    pub cpu_temp: Option<f32>,
    pub battery: Option<Battery>,
    /// kB
    pub mem_total: u64,
    pub mem_avail: u64,
    pub swap_total: u64,
    pub swap_free: u64,
    pub net: Option<(String, String)>,
    pub disks: Vec<Disk>,
}

impl Info {
    pub fn mem_used(&self) -> u64 {
        self.mem_total.saturating_sub(self.mem_avail)
    }
    pub fn mem_pct(&self) -> f32 {
        pct(self.mem_used(), self.mem_total)
    }
    pub fn swap_used(&self) -> u64 {
        self.swap_total.saturating_sub(self.swap_free)
    }
    pub fn swap_pct(&self) -> f32 {
        pct(self.swap_used(), self.swap_total)
    }
    /// Media de carga (1, 5 e 15 min) em relacao ao numero de nucleos, como no script.
    pub fn load_pct(&self) -> f32 {
        let avg = (self.load[0] + self.load[1] + self.load[2]) / 3.0;
        avg / self.cores.max(1) as f32 * 100.0
    }
}

pub fn pct(used: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        used as f32 * 100.0 / total as f32
    }
}

// ---------------------------------------------------------------------------
// Parsers puros
// ---------------------------------------------------------------------------

pub fn parse_uptime(s: &str) -> u64 {
    s.split_whitespace()
        .next()
        .and_then(|v| v.parse::<f64>().ok())
        .map_or(0, |v| v as u64)
}

pub fn parse_loadavg(s: &str) -> [f32; 3] {
    let mut it = s.split_whitespace().map(|v| v.parse().unwrap_or(0.0));
    [
        it.next().unwrap_or(0.0),
        it.next().unwrap_or(0.0),
        it.next().unwrap_or(0.0),
    ]
}

/// Valor em kB de uma linha `Chave:   123 kB` do /proc/meminfo.
pub fn meminfo(s: &str, key: &str) -> u64 {
    s.lines()
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            (k == key).then(|| v.split_whitespace().next()?.parse().ok())?
        })
        .unwrap_or(0)
}

/// `model name` (x86) ou `Hardware` (ARM antigo); em ARM moderno nenhum existe e o
/// chamador cai no `Model` do device-tree.
pub fn parse_cpu_model(s: &str) -> Option<String> {
    s.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        let k = k.trim();
        (k == "model name" || k == "Hardware").then(|| v.trim().to_string())
    })
}

pub fn count_cores(cpuinfo: &str) -> usize {
    cpuinfo
        .lines()
        .filter(|l| l.starts_with("processor"))
        .count()
}

/// Dispositivo montado exatamente em `mount`; a ultima linha vence (montagens empilhadas).
pub fn mount_device(mounts: &str, mount: &str) -> Option<String> {
    mounts
        .lines()
        .filter_map(|l| {
            let mut f = l.split_whitespace();
            let (dev, mp) = (f.next()?, f.next()?);
            (mp == mount).then(|| dev.to_string())
        })
        .next_back()
}

/// Nomes dos usuarios logados (registros USER_PROCESS do utmp, 384 bytes cada em
/// glibc x86_64/aarch64), como o `who` do script.
pub fn parse_utmp(b: &[u8]) -> Vec<String> {
    const REC: usize = 384;
    b.chunks_exact(REC)
        .filter(|r| i16::from_ne_bytes([r[0], r[1]]) == 7)
        .map(|r| {
            let n = &r[44..76];
            let end = n.iter().position(|&c| c == 0).unwrap_or(n.len());
            String::from_utf8_lossy(&n[..end]).into_owned()
        })
        .collect()
}

pub fn fmt_uptime(s: u64) -> String {
    let (d, h, m) = (s / 86400, s % 86400 / 3600, s % 3600 / 60);
    match (d, h) {
        (0, 0) => format!("{m}m"),
        (0, _) => format!("{h}h {m}m"),
        _ => format!("{d}d {h}h {m}m"),
    }
}

/// kB -> "1.9 GB" / "512 MB".
pub fn fmt_kb(kb: u64) -> String {
    let mb = kb as f64 / 1024.0;
    if mb >= 1024.0 {
        format!("{:.1} GB", mb / 1024.0)
    } else {
        format!("{mb:.0} MB")
    }
}

/// bytes -> "58.2 GB", base 1024 (como o `df -h`).
pub fn fmt_bytes(b: u64) -> String {
    const U: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let (mut v, mut i) = (b as f64, 0);
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{b} B")
    } else if v >= 100.0 {
        format!("{v:.0} {}", U[i])
    } else {
        format!("{v:.1} {}", U[i])
    }
}

// ---------------------------------------------------------------------------
// Coleta
// ---------------------------------------------------------------------------

pub struct Sys {
    root: PathBuf,
}

impl Default for Sys {
    fn default() -> Self {
        Sys::new("/")
    }
}

impl Sys {
    pub fn new(root: impl Into<PathBuf>) -> Sys {
        Sys { root: root.into() }
    }

    fn p(&self, rel: &str) -> PathBuf {
        self.root.join(rel.trim_start_matches('/'))
    }

    fn read(&self, rel: &str) -> Option<String> {
        fs::read_to_string(self.p(rel)).ok()
    }

    fn read_trim(&self, rel: &str) -> Option<String> {
        self.read(rel).map(|s| s.trim().to_string())
    }

    fn num(&self, rel: &str) -> Option<f64> {
        self.read_trim(rel)?.parse().ok()
    }

    /// Zona termica cujo `type` contem alguma das palavras; senao o indice do script.
    fn thermal(&self, words: &[&str], fallback: usize) -> (Option<f32>, String) {
        let dir = self.p("sys/class/thermal");
        let mut zones: Vec<String> = fs::read_dir(dir)
            .map(|d| {
                d.filter_map(|e| e.ok()?.file_name().into_string().ok())
                    .filter(|n| n.starts_with("thermal_zone"))
                    .collect()
            })
            .unwrap_or_default();
        zones.sort_by_key(|n| {
            n.trim_start_matches("thermal_zone")
                .parse::<u32>()
                .unwrap_or(0)
        });
        let kind = |z: &str| {
            self.read_trim(&format!("sys/class/thermal/{z}/type"))
                .unwrap_or_default()
        };
        let pick = zones
            .iter()
            .find(|z| {
                let k = kind(z).to_lowercase();
                words.iter().any(|w| k.contains(w))
            })
            .cloned()
            .or_else(|| {
                let f = format!("thermal_zone{fallback}");
                zones.contains(&f).then_some(f)
            });
        match pick {
            Some(z) => (
                self.num(&format!("sys/class/thermal/{z}/temp"))
                    .map(|t| t as f32 / 1000.0),
                kind(&z),
            ),
            None => (None, String::new()),
        }
    }

    fn battery(&self) -> Option<Battery> {
        let base = "sys/class/power_supply";
        let mut dir = format!("{base}/battery");
        if !self.p(&dir).is_dir() {
            let found = fs::read_dir(self.p(base)).ok()?.filter_map(|e| {
                let n = e.ok()?.file_name().into_string().ok()?;
                let t = self.read_trim(&format!("{base}/{n}/type"))?;
                (t == "Battery").then_some(n)
            });
            dir = format!("{base}/{}", found.min()?);
        }
        let (temp, _) = self.thermal(&["batt"], 0);
        Some(Battery {
            capacity: self
                .num(&format!("{dir}/capacity"))
                .map(|c| c.clamp(0.0, 100.0) as u8),
            status: self.read_trim(&format!("{dir}/status")).unwrap_or_default(),
            // O script original rotula estes valores como uV/uA, mas divide por 1e6: sao V e A.
            voltage: self
                .num(&format!("{dir}/voltage_now"))
                .map(|v| v as f32 / 1e6),
            current: self
                .num(&format!("{dir}/current_now"))
                .map(|v| v as f32 / 1e6),
            temp,
        })
    }

    fn disk(&self, path: &str, mounts: &str) -> Disk {
        let c = CString::new(self.p(path).to_string_lossy().into_owned()).unwrap_or_default();
        // SAFETY: `s` e uma struct de saida zerada e `c` e uma string C valida.
        let (total, used) = unsafe {
            let mut s: libc::statvfs = std::mem::zeroed();
            if libc::statvfs(c.as_ptr(), &mut s) == 0 {
                let fr = s.f_frsize as u64;
                let total = s.f_blocks as u64 * fr;
                (total, total.saturating_sub(s.f_bfree as u64 * fr))
            } else {
                (0, 0)
            }
        };
        Disk {
            path: path.to_string(),
            fs: mount_device(mounts, path),
            total,
            used,
        }
    }

    pub fn collect(&self) -> Info {
        let cpuinfo = self.read("proc/cpuinfo").unwrap_or_default();
        let mem = self.read("proc/meminfo").unwrap_or_default();
        let mounts = self.read("proc/mounts").unwrap_or_default();
        let (cpu_temp, cpu_type) = self.thermal(&["cpu", "soc"], 1);
        let model = parse_cpu_model(&cpuinfo)
            .or_else(|| {
                self.read("proc/device-tree/model")
                    .map(|s| s.trim_end_matches('\0').trim().to_string())
            })
            .unwrap_or_default();
        Info {
            host: self
                .read_trim("proc/sys/kernel/hostname")
                .unwrap_or_default(),
            users: fs::read(self.p("var/run/utmp"))
                .or_else(|_| fs::read(self.p("run/utmp")))
                .map(|b| parse_utmp(&b))
                .unwrap_or_default(),
            uptime_s: parse_uptime(&self.read("proc/uptime").unwrap_or_default()),
            load: parse_loadavg(&self.read("proc/loadavg").unwrap_or_default()),
            cores: count_cores(&cpuinfo).max(1),
            cpu_model: model,
            cpu_type,
            cpu_temp,
            battery: self.battery(),
            mem_total: meminfo(&mem, "MemTotal"),
            mem_avail: meminfo(&mem, "MemAvailable"),
            swap_total: meminfo(&mem, "SwapTotal"),
            swap_free: meminfo(&mem, "SwapFree"),
            net: if self.root == Path::new("/") {
                primary_ipv4()
            } else {
                None
            },
            disks: ["/", "/roms", "/roms2"]
                .iter()
                .map(|p| self.disk(p, &mounts))
                .collect(),
        }
    }
}

/// Primeira interface IPv4 ativa que nao seja loopback: (nome, endereco).
pub fn primary_ipv4() -> Option<(String, String)> {
    // SAFETY: getifaddrs preenche uma lista encadeada que liberamos com freeifaddrs;
    // so lemos campos enquanto ela existe.
    unsafe {
        let mut ifa: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut ifa) != 0 {
            return None;
        }
        let mut found = None;
        let mut cur = ifa;
        while !cur.is_null() && found.is_none() {
            let a = &*cur;
            let up = a.ifa_flags & libc::IFF_UP as u32 != 0
                && a.ifa_flags & libc::IFF_LOOPBACK as u32 == 0;
            if up && !a.ifa_addr.is_null() && (*a.ifa_addr).sa_family as i32 == libc::AF_INET {
                let sin = &*(a.ifa_addr as *const libc::sockaddr_in);
                let ip = u32::from_be(sin.sin_addr.s_addr);
                let name = std::ffi::CStr::from_ptr(a.ifa_name)
                    .to_string_lossy()
                    .into_owned();
                found = Some((
                    name,
                    format!(
                        "{}.{}.{}.{}",
                        ip >> 24,
                        ip >> 16 & 255,
                        ip >> 8 & 255,
                        ip & 255
                    ),
                ));
            }
            cur = a.ifa_next;
        }
        libc::freeifaddrs(ifa);
        found
    }
}

/// Data/hora local, ex.: ("Tue 03 Nov 2026", "21:29:05").
pub fn local_time() -> (String, String) {
    const D: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const M: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    // SAFETY: localtime_r escreve em `tm`, que e nossa.
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        (
            format!(
                "{} {:02} {} {}",
                D[tm.tm_wday.rem_euclid(7) as usize],
                tm.tm_mday,
                M[tm.tm_mon.rem_euclid(12) as usize],
                tm.tm_year + 1900
            ),
            format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsers_basicos() {
        assert_eq!(parse_uptime("12345.67 9999.1\n"), 12345);
        assert_eq!(parse_uptime(""), 0);
        assert_eq!(
            parse_loadavg("0.50 1.25 2.00 1/200 999\n"),
            [0.5, 1.25, 2.0]
        );
        assert_eq!(parse_loadavg("lixo"), [0.0; 3]);
        let m = "MemTotal:        2000000 kB\nMemAvailable:    500000 kB\nSwapTotal: 0 kB\n";
        assert_eq!(meminfo(m, "MemTotal"), 2_000_000);
        assert_eq!(meminfo(m, "MemAvailable"), 500_000);
        assert_eq!(meminfo(m, "SwapFree"), 0);
        // "MemTotal" nao casa com "MemTotalX"
        assert_eq!(meminfo("MemTotalX: 5 kB", "MemTotal"), 0);
    }

    #[test]
    fn cpu_e_montagens() {
        let c = "processor\t: 0\nmodel name\t: ARMv8 rev 2\nprocessor\t: 1\nHardware\t: X\n";
        assert_eq!(parse_cpu_model(c).as_deref(), Some("ARMv8 rev 2"));
        assert_eq!(count_cores(c), 2);
        assert_eq!(parse_cpu_model("processor: 0\n"), None);
        let mt = "/dev/root / ext4 rw 0 0\n/dev/mmcblk1p3 /roms exfat rw 0 0\n/dev/a /roms ext4 rw 0 0\n";
        assert_eq!(mount_device(mt, "/").as_deref(), Some("/dev/root"));
        assert_eq!(mount_device(mt, "/roms").as_deref(), Some("/dev/a")); // a ultima vence
        assert_eq!(mount_device(mt, "/roms2"), None);
    }

    #[test]
    fn utmp_conta_so_sessoes_de_usuario() {
        let mut b = vec![0u8; 384 * 3];
        b[0..2].copy_from_slice(&7i16.to_ne_bytes()); // USER_PROCESS
        b[44..47].copy_from_slice(b"ark");
        b[384..386].copy_from_slice(&8i16.to_ne_bytes()); // DEAD_PROCESS
        b[384 + 44..384 + 48].copy_from_slice(b"gone");
        b[768..770].copy_from_slice(&7i16.to_ne_bytes());
        b[768 + 44..768 + 48].copy_from_slice(b"root");
        assert_eq!(parse_utmp(&b), ["ark", "root"]);
        assert!(parse_utmp(&[1, 2, 3]).is_empty());
    }

    #[test]
    fn formatacao() {
        assert_eq!(fmt_uptime(59), "0m");
        assert_eq!(fmt_uptime(3 * 3600 + 5 * 60), "3h 5m");
        assert_eq!(fmt_uptime(2 * 86400 + 3600), "2d 1h 0m");
        assert_eq!(fmt_kb(512 * 1024), "512 MB");
        assert_eq!(fmt_kb(1536 * 1024), "1.5 GB");
        assert_eq!(fmt_bytes(500), "500 B");
        assert_eq!(fmt_bytes(62 * 1024 * 1024 * 1024), "62.0 GB");
        assert_eq!(fmt_bytes(120 * 1024 * 1024 * 1024), "120 GB");
        assert_eq!(pct(1, 0), 0.0);
        assert!((pct(1, 4) - 25.0).abs() < 1e-6);
    }

    #[test]
    fn calculos_derivados_como_no_script() {
        let i = Info {
            load: [1.0, 2.0, 3.0],
            cores: 4,
            mem_total: 1000,
            mem_avail: 250,
            swap_total: 100,
            swap_free: 100,
            ..Info::default()
        };
        assert!((i.load_pct() - 50.0).abs() < 1e-4); // media 2.0 / 4 nucleos
        assert_eq!(i.mem_used(), 750);
        assert!((i.mem_pct() - 75.0).abs() < 1e-4);
        assert_eq!(i.swap_pct(), 0.0);
        assert_eq!(Info::default().load_pct(), 0.0); // sem divisao por zero
    }

    fn fake_root(name: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!("system_info-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&r);
        let w = |p: &str, c: &str| {
            let f = r.join(p);
            fs::create_dir_all(f.parent().unwrap()).unwrap();
            fs::write(f, c).unwrap();
        };
        w("proc/uptime", "7384.5 1.0\n");
        w("proc/loadavg", "0.30 0.60 0.90 1/100 5\n");
        w(
            "proc/cpuinfo",
            "processor\t: 0\nprocessor\t: 1\nHardware\t: RK3562\n",
        );
        w("proc/meminfo", "MemTotal: 2048000 kB\nMemAvailable: 1024000 kB\nSwapTotal: 1000 kB\nSwapFree: 400 kB\n");
        w("proc/sys/kernel/hostname", "rg52\n");
        w(
            "proc/mounts",
            "/dev/root / ext4 rw 0 0\n/dev/sda1 /roms exfat rw 0 0\n",
        );
        w("sys/class/thermal/thermal_zone0/type", "battery\n");
        w("sys/class/thermal/thermal_zone0/temp", "31500\n");
        w("sys/class/thermal/thermal_zone1/type", "soc-thermal\n");
        w("sys/class/thermal/thermal_zone1/temp", "47000\n");
        w("sys/class/power_supply/battery/capacity", "83\n");
        w("sys/class/power_supply/battery/status", "Discharging\n");
        w("sys/class/power_supply/battery/voltage_now", "3987000\n");
        w("sys/class/power_supply/battery/current_now", "-450000\n");
        r
    }

    #[test]
    fn coleta_de_uma_arvore_falsa() {
        let r = fake_root("full");
        let i = Sys::new(&r).collect();
        assert_eq!(i.host, "rg52");
        assert_eq!(i.uptime_s, 7384);
        assert_eq!(i.cores, 2);
        assert_eq!(i.cpu_model, "RK3562");
        assert_eq!(i.cpu_type, "soc-thermal");
        assert_eq!(i.cpu_temp, Some(47.0));
        assert_eq!((i.mem_total, i.mem_avail), (2_048_000, 1_024_000));
        assert!((i.mem_pct() - 50.0).abs() < 1e-3);
        assert!((i.swap_pct() - 60.0).abs() < 1e-3);
        let b = i.battery.unwrap();
        assert_eq!(b.capacity, Some(83));
        assert_eq!(b.status, "Discharging");
        assert!((b.voltage.unwrap() - 3.987).abs() < 1e-4);
        assert!((b.current.unwrap() + 0.45).abs() < 1e-4);
        assert_eq!(b.temp, Some(31.5));
        assert_eq!(i.disks.len(), 3);
        assert_eq!(i.disks[0].fs.as_deref(), Some("/dev/root"));
        assert_eq!(i.disks[2].fs, None); // /roms2 nao montado
        assert!(i.net.is_none()); // so consulta a rede na raiz real
        let _ = fs::remove_dir_all(r);
    }

    #[test]
    fn arvore_vazia_nao_quebra() {
        let r = std::env::temp_dir().join(format!("system_info-empty-{}", std::process::id()));
        let _ = fs::create_dir_all(&r);
        let i = Sys::new(&r).collect();
        assert_eq!(i.host, "");
        assert_eq!(i.cores, 1);
        assert_eq!(i.cpu_temp, None);
        assert!(i.battery.is_none());
        assert_eq!(i.mem_pct(), 0.0);
        let _ = fs::remove_dir_all(r);
    }

    #[test]
    fn bateria_alternativa_por_tipo_e_zona_por_nome() {
        let r = fake_root("alt");
        fs::rename(
            r.join("sys/class/power_supply/battery"),
            r.join("sys/class/power_supply/bq27"),
        )
        .unwrap();
        fs::write(r.join("sys/class/power_supply/bq27/type"), "Battery\n").unwrap();
        // zonas em ordem trocada: a escolha e pelo nome, nao pelo indice
        fs::write(
            r.join("sys/class/thermal/thermal_zone0/type"),
            "cpu-thermal\n",
        )
        .unwrap();
        fs::write(r.join("sys/class/thermal/thermal_zone0/temp"), "55000\n").unwrap();
        let i = Sys::new(&r).collect();
        assert_eq!(i.battery.unwrap().capacity, Some(83));
        assert_eq!(i.cpu_temp, Some(55.0));
        let _ = fs::remove_dir_all(r);
    }

    #[test]
    fn disco_real_e_hora_local() {
        let d = Sys::default().disk("/", "");
        assert!(d.total > 0 && d.used <= d.total);
        let (date, time) = local_time();
        assert_eq!(time.len(), 8);
        assert!(date.len() >= 15);
    }
}
