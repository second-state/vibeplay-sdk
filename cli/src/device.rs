//! 和掌机说话：配对、推游戏。走局域网的 HTTP，到掌机上 QingOS 的控制台（只在开发者模式下开）。
//!
//! ```text
//! GET  /api/apps/push     头 x-qing-code: <口令>   → 200 {"device": 名字, "apps": [名字…]}
//! POST /api/apps/push     头 x-qing-code，身子是 .vp 包
//!                  → 200 {"name": 名字, "state": "updated" | "installed" | "declined" | "timeout"}
//! ```
//!
//! 同名的游戏直接换掉（存档留着）再打开。新游戏掌机上问一次「装不装」，问的时候这个请求等着。
//! 口令存在 ~/.config/vibeplay/device.json：只在这台开发机上，不进仓库。

use crate::{pack, pick};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

#[derive(Serialize, Deserialize, Clone)]
pub struct Device {
    pub host: String,
    pub code: String,
}

fn file() -> Option<PathBuf> {
    Some(config_dir()?.join("device.json"))
}

/// 命令行存东西的目录：~/.config/vibeplay。只在这台开发机上。
pub fn config_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("vibeplay"))
}

/// 配过的掌机。
pub fn saved() -> Option<Device> {
    serde_json::from_slice(&std::fs::read(file()?).ok()?).ok()
}

fn base(host: &str) -> String {
    let h = host.trim().trim_end_matches('/');
    if h.starts_with("http://") || h.starts_with("https://") {
        h.to_string()
    } else {
        format!("http://{h}")
    }
}

fn agent(secs: u64) -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(secs))).http_status_as_error(false).build().into()
}

/// 问一下掌机：回 (状态码, 身子)。
fn hello(d: &Device) -> Result<(u16, serde_json::Value)> {
    let mut r = agent(5).get(format!("{}/api/apps/push", base(&d.host))).header("x-qing-code", &d.code).call().with_context(|| pick(format!("连不上 {}：掌机开着吗？和开发机在一个 Wi‑Fi 里吗？", d.host), format!("Cannot connect to {}. Is the handheld on? Is it on the same Wi‑Fi as this computer?", d.host)))?;
    let status = r.status().as_u16();
    let body = r.body_mut().read_json().unwrap_or(serde_json::Value::Null);
    Ok((status, body))
}

/// 状态码说成人话。
fn explain(status: u16, host: &str) -> anyhow::Error {
    match status {
        401 => anyhow::anyhow!(pick(format!("口令不对：看掌机屏幕上的口令（设置 → 系统），再 vibeplay pair {host}"), format!("The code is wrong. Read the code on the handheld screen (Settings → System), then run vibeplay pair {host} again."))),
        404 => anyhow::anyhow!(pick("掌机没开开发者模式：在掌机的设置 → 系统里打开，重启后再试", "Developer mode is off on the handheld. Turn it on in Settings → System, restart the handheld, then try again.")),
        413 => anyhow::anyhow!(pick("包太大了，掌机不收", "The package is too large. The handheld does not accept it.")),
        s => anyhow::anyhow!(pick(format!("掌机回了 {s}"), format!("The handheld replied {s}"))),
    }
}

pub fn reachable(d: &Device) -> bool {
    matches!(hello(d), Ok((200, _)))
}

/// 配对：口令对了就存下来。
pub fn pair(host: &str, code: Option<&str>) -> Result<()> {
    let code = match code {
        Some(c) => c.to_string(),
        None => {
            eprint!("{}", pick("掌机屏幕上的口令（设置 → 系统）：", "Code on the handheld screen (Settings → System): "));
            let mut s = String::new();
            std::io::stdin().read_line(&mut s)?;
            s.trim().to_string()
        }
    };
    let d = Device { host: host.trim().to_string(), code };
    let (status, body) = hello(&d)?;
    if status != 200 {
        return Err(explain(status, host));
    }
    let f = file().context(pick("没有 HOME，存不了口令", "HOME is not set. Cannot save the code."))?;
    std::fs::create_dir_all(f.parent().unwrap())?;
    std::fs::write(&f, serde_json::to_vec_pretty(&d)?)?;
    let games: Vec<&str> = body["apps"].as_array().into_iter().flatten().filter_map(|v| v.as_str()).collect();
    let device = body["device"].as_str().unwrap_or(pick("掌机", "handheld"));
    println!(
        "{}",
        pick(
            format!("配好了：{}（{}）。上面有 {} 个游戏{}", device, d.host, games.len(), if games.is_empty() { String::new() } else { format!("：{}", games.join("、")) }),
            format!("Paired: {} ({}). It has {} games{}", device, d.host, games.len(), if games.is_empty() { String::new() } else { format!(": {}", games.join(", ")) })
        )
    );
    println!("{}", pick("vibeplay push 推当前目录的游戏。", "Run vibeplay push to send the game in this directory."));
    Ok(())
}

/// 推一次：打包、发过去、说结果。
fn push_once(dir: &Path, d: &Device) -> Result<()> {
    let (bytes, g) = pack::pack(dir, Some(pack::DEVICE_TARGET))?;
    println!("{}", pick(format!("推 {}（{} KB）到 {}…", g.m.name, bytes.len() / 1024, d.host), format!("Sending {} ({} KB) to {}…", g.m.name, bytes.len() / 1024, d.host)));
    // 新游戏要人在掌机上按，最多等 90 秒。
    let mut r = agent(90)
        .post(format!("{}/api/apps/push", base(&d.host)))
        .header("x-qing-code", &d.code)
        .header("content-type", "application/x-vibeplay")
        .send(&bytes[..])
        .with_context(|| pick(format!("连不上 {}", d.host), format!("Cannot connect to {}", d.host)))?;
    let status = r.status().as_u16();
    if status != 200 {
        let text = r.body_mut().read_to_string().unwrap_or_default();
        return Err(explain(status, &d.host).context(text));
    }
    let body: serde_json::Value = r.body_mut().read_json().context(pick("掌机回的不是 JSON", "The handheld did not reply with JSON"))?;
    match body["state"].as_str().unwrap_or("") {
        "updated" => println!("{}", pick(format!("换好了，掌机上开着 {}（存档留着）", g.m.name), format!("Updated. {} is open on the handheld (the saves stay).", g.m.name))),
        "installed" => println!("{}", pick(format!("装好了，掌机上开着 {}", g.m.name), format!("Installed. {} is open on the handheld.", g.m.name))),
        "declined" => bail!(pick("掌机上按了「不装」", "On the handheld, someone selected Skip.")),
        "timeout" => bail!(pick("掌机上没人按：新游戏要在掌机上按 A 才装", "No one pushed a button on the handheld. To install a new game, push A on the handheld.")),
        s => bail!(pick(format!("掌机回了不认识的 {s}"), format!("The handheld replied with an unknown state: {s}"))),
    }
    Ok(())
}

/// 游戏目录里最新的改动时间：源码、素材、清单。
fn newest(dir: &Path) -> SystemTime {
    fn walk(p: &Path, best: &mut SystemTime) {
        let Ok(m) = std::fs::metadata(p) else { return };
        if m.is_dir() {
            for e in std::fs::read_dir(p).into_iter().flatten().flatten() {
                walk(&e.path(), best);
            }
        } else if let Ok(t) = m.modified() {
            *best = (*best).max(t);
        }
    }
    let mut best = SystemTime::UNIX_EPOCH;
    for rel in ["src", "assets", "app.toml", "Cargo.toml"] {
        walk(&dir.join(rel), &mut best);
    }
    best
}

pub fn push(dir: &Path, host: Option<&str>, watch: bool) -> Result<()> {
    let mut d = saved().context(pick("还没配对：vibeplay pair <掌机的地址>", "Not paired yet. Run vibeplay pair <handheld address>."))?;
    if let Some(h) = host {
        d.host = h.to_string();
    }
    if !watch {
        return push_once(dir, &d);
    }
    println!("{}", pick(format!("盯着 {}：一改就再推。Ctrl‑C 停。", dir.display()), format!("Watching {}. Each change sends the game again. Push Ctrl‑C to stop.", dir.display())));
    let mut seen = SystemTime::UNIX_EPOCH;
    loop {
        let t = newest(dir);
        if t > seen {
            seen = t;
            if let Err(e) = push_once(dir, &d) {
                eprintln!("{}", pick(format!("vibeplay：{e:#}"), format!("vibeplay: {e:#}")));
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}
