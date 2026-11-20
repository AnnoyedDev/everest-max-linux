use std::path::PathBuf;
use std::time::Instant;

pub struct Stats {
    cpu: (u64, u64),
    disk: u64,
    net: (String, u64, u64),
    at: Instant,
    gpu_sysfs: Option<PathBuf>,
    nvidia: bool,
}

fn read(p: &str) -> String {
    std::fs::read_to_string(p).unwrap_or_default()
}

fn cpu_times() -> (u64, u64) {
    let s = read("/proc/stat");
    let v: Vec<u64> = s.lines().next().unwrap_or("").split_whitespace().skip(1).filter_map(|x| x.parse().ok()).collect();
    let total = v.iter().take(8).sum();
    let idle = v.get(3).unwrap_or(&0) + v.get(4).unwrap_or(&0);
    (total, idle)
}

fn disk_ticks() -> u64 {
    let Ok(dir) = std::fs::read_dir("/sys/block") else { return 0 };
    dir.flatten()
        .filter(|d| {
            let n = d.file_name().to_string_lossy().to_string();
            !["loop", "ram", "zram", "dm-", "sr"].iter().any(|p| n.starts_with(p))
        })
        .filter_map(|d| std::fs::read_to_string(d.path().join("stat")).ok())
        .filter_map(|s| s.split_whitespace().nth(9).and_then(|x| x.parse::<u64>().ok()))
        .max()
        .unwrap_or(0)
}

fn net_counter(iface: &str, name: &str) -> u64 {
    read(&format!("/sys/class/net/{iface}/statistics/{name}")).trim().parse().unwrap_or(0)
}

pub fn default_iface() -> Option<String> {
    read("/proc/net/route").lines().skip(1)
        .find(|l| l.split_whitespace().nth(1) == Some("00000000"))
        .and_then(|l| l.split_whitespace().next().map(String::from))
}

pub fn link_speed(iface: &str) -> Option<u32> {
    read(&format!("/sys/class/net/{iface}/speed")).trim().parse::<i64>().ok().filter(|&s| s > 0).map(|s| s as u32)
}

pub fn interfaces() -> Vec<(String, Option<u32>)> {
    let mut v: Vec<(String, Option<u32>)> = std::fs::read_dir("/sys/class/net").into_iter().flatten().flatten()
        .map(|d| d.file_name().to_string_lossy().to_string())
        .filter(|n| !["lo", "veth", "docker", "br-", "virbr", "tun", "tap"].iter().any(|p| n.starts_with(p)))
        .map(|n| { let s = link_speed(&n); (n, s) })
        .collect();
    v.sort();
    v
}

pub struct NetCfg<'a> {
    pub iface: &'a str,
    pub dir: u8,
    pub max_mbps: u32,
}

impl NetCfg<'_> {
    pub fn resolve(&self) -> (String, u32) {
        let iface = if self.iface.is_empty() { default_iface().unwrap_or_default() } else { self.iface.to_string() };
        let max = if self.max_mbps > 0 { self.max_mbps } else { link_speed(&iface).unwrap_or(1000) };
        (iface, max.max(1))
    }
}

fn ram_percent() -> u16 {
    let s = read("/proc/meminfo");
    let get = |k: &str| s.lines().find(|l| l.starts_with(k)).and_then(|l| l.split_whitespace().nth(1)?.parse::<f64>().ok()).unwrap_or(0.0);
    let (total, avail) = (get("MemTotal:"), get("MemAvailable:"));
    if total > 0.0 { ((1.0 - avail / total) * 100.0).round() as u16 } else { 0 }
}

impl Stats {
    pub fn new() -> Stats {
        let gpu_sysfs = std::fs::read_dir("/sys/class/drm").ok().into_iter().flatten().flatten()
            .map(|d| d.path().join("device/gpu_busy_percent")).find(|p| p.exists());
        let nvidia = gpu_sysfs.is_none() && std::process::Command::new("nvidia-smi").arg("-L").output().map(|o| o.status.success()).unwrap_or(false);
        Stats { cpu: cpu_times(), disk: disk_ticks(), net: (String::new(), 0, 0), at: Instant::now(), gpu_sysfs, nvidia }
    }

    fn gpu(&self) -> u16 {
        if let Some(p) = &self.gpu_sysfs {
            return std::fs::read_to_string(p).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
        }
        if self.nvidia {
            return std::process::Command::new("nvidia-smi").args(["--query-gpu=utilization.gpu", "--format=csv,noheader,nounits"]).output().ok()
                .and_then(|o| String::from_utf8_lossy(&o.stdout).lines().next()?.trim().parse().ok()).unwrap_or(0);
        }
        0
    }

    pub fn sample(&mut self, net: &NetCfg) -> [u16; 5] {
        let dt = self.at.elapsed().as_secs_f64().max(0.05);
        self.at = Instant::now();
        let (total, idle) = cpu_times();
        let (dtot, didle) = (total.saturating_sub(self.cpu.0), idle.saturating_sub(self.cpu.1));
        self.cpu = (total, idle);
        let cpu = if dtot > 0 { (100.0 * (1.0 - didle as f64 / dtot as f64)).round() as u16 } else { 0 };
        let ticks = disk_ticks();
        let disk = ((ticks.saturating_sub(self.disk)) as f64 / (dt * 1000.0) * 100.0).round().min(100.0) as u16;
        self.disk = ticks;
        let (iface, max) = net.resolve();
        let (rx, tx) = (net_counter(&iface, "rx_bytes"), net_counter(&iface, "tx_bytes"));
        let pct = if self.net.0 == iface {
            let (drx, dtx) = (rx.saturating_sub(self.net.1), tx.saturating_sub(self.net.2));
            let bytes = match net.dir { 0 => drx, 1 => dtx, _ => drx.max(dtx) };
            (bytes as f64 * 8.0 / 1e6 / dt / max as f64 * 100.0).round().min(100.0) as u16
        } else {
            0
        };
        self.net = (iface, rx, tx);
        [cpu.min(100), self.gpu().min(100), disk, pct, ram_percent().min(100)]
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn sample_is_bounded() {
        let mut s = super::Stats::new();
        std::thread::sleep(std::time::Duration::from_millis(120));
        let net = super::NetCfg { iface: "", dir: 2, max_mbps: 0 };
        s.sample(&net);
        std::thread::sleep(std::time::Duration::from_millis(120));
        assert!(s.sample(&net).iter().all(|&v| v <= 100));
    }
}
