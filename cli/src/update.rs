//! 新版本：命令跑完提示一句（一天最多查一次），`vibeplay update` 替人重装。
//!
//! 装命令行是 `cargo install --git`，装的是 main 上的那一版。所以最新的版本就是 main 上 Cargo.toml 写的版本。
//! 查的时候只取那个文件，不带任何关于这台电脑、这个人的东西。查过的结果存在 ~/.config/vibeplay/update.json。
//! 设 VIBEPLAY_NO_UPDATE_CHECK=1 就不查；CI 里（有 CI 环境变量）也不查。

use crate::pick;
use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const REPO: &str = "https://github.com/second-state/vibeplay-sdk";
const CARGO_TOML: &str = "https://raw.githubusercontent.com/second-state/vibeplay-sdk/main/Cargo.toml";
/// 隔多久查一次。
const EVERY: u64 = 24 * 3600;

/// 这台装的版本。
fn current() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// 重装用的那一行命令。
fn install_line() -> String {
    format!("cargo install --git {REPO} vibeplay-cli --force")
}

/// 从 Cargo.toml 读 `[workspace.package]` 的 version。
fn version_in(cargo_toml: &str) -> Option<String> {
    let v: toml::Value = toml::from_str(cargo_toml).ok()?;
    Some(v.get("workspace")?.get("package")?.get("version")?.as_str()?.to_string())
}

/// `主.次.补` 比大小。写错的当 0。`a` 比 `b` 新吗。
fn newer(a: &str, b: &str) -> bool {
    let n = |s: &str| -> [u64; 3] {
        let mut r = [0; 3];
        for (x, p) in r.iter_mut().zip(s.split('-').next().unwrap_or("").split('.')) {
            *x = p.parse().unwrap_or(0);
        }
        r
    };
    n(a) > n(b)
}

/// 取 main 上的版本。`secs` 秒没回就算了。
fn latest(secs: u64) -> Result<String> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(secs))).build().into();
    let body = agent.get(CARGO_TOML).call()?.body_mut().read_to_string()?;
    version_in(&body).context("Cargo.toml has no [workspace.package] version")
}

fn cache() -> Option<PathBuf> {
    Some(crate::device::config_dir()?.join("update.json"))
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// 命令跑完调：有新版就在 stderr 上说一句（stdout 留给命令自己的输出）。上次查过不到一天就用上次的结果。
/// 查不了（没网、超时）什么都不说，明天再查。
pub fn hint() {
    if std::env::var_os("VIBEPLAY_NO_UPDATE_CHECK").is_some_and(|v| !v.is_empty()) || std::env::var_os("CI").is_some() {
        return;
    }
    let Some(path) = cache() else { return };
    let saved: Option<serde_json::Value> = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok());
    let fresh = saved.as_ref().and_then(|v| v["checked"].as_u64()).is_some_and(|t| now().saturating_sub(t) < EVERY);
    let latest = if fresh {
        saved.as_ref().and_then(|v| v["latest"].as_str().map(String::from))
    } else {
        let got = latest(2).ok();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, serde_json::json!({ "checked": now(), "latest": got }).to_string());
        got
    };
    if let Some(l) = latest.filter(|l| newer(l, current())) {
        eprintln!(
            "{}",
            pick(
                format!("\nvibeplay {l} 出来了（这台是 {}）。升级：vibeplay update", current()),
                format!("\nvibeplay {l} is out (this computer has {}). To update: vibeplay update", current())
            )
        );
    }
}

/// `vibeplay update`：main 上的版本比这台新，就跑 `cargo install --git … --force` 重装。
pub fn run() -> Result<()> {
    match latest(15) {
        Ok(l) if !newer(&l, current()) => {
            println!("{}", pick(format!("已经是最新的：{}", current()), format!("Already up to date: {}", current())));
            return Ok(());
        }
        Ok(l) => println!("{}", pick(format!("升级 {} → {l}：{}", current(), install_line()), format!("Updating {} → {l}: {}", current(), install_line()))),
        // 查不了也能装：装的是 main 上的那一版。
        Err(e) => println!("{}", pick(format!("查不了最新的版本（{e}），照样重装：{}", install_line()), format!("Cannot check the latest version ({e}). Reinstalling anyway: {}", install_line()))),
    }
    let ok = std::process::Command::new("cargo")
        .args(["install", "--git", REPO, "vibeplay-cli", "--force"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        bail!(pick(format!("没装上。自己在终端里跑：{}", install_line()), format!("The update failed. Run this in your terminal: {}", install_line())));
    }
    // 存的结果作废：下次命令跑完重新查。
    if let Some(p) = cache() {
        let _ = std::fs::remove_file(p);
    }
    println!(
        "{}",
        pick(
            "装好了。给 agent 的说明可能也改了：在你自己的终端里再跑一次 vibeplay skill install，然后重开 Claude Code 或 Codex。",
            "Done. The instructions for agents can also change: run vibeplay skill install again in your own terminal, then restart Claude Code or Codex."
        )
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare() {
        assert!(newer("0.4.2", "0.4.1"));
        assert!(newer("0.10.0", "0.9.9"));
        assert!(newer("1.0.0", "0.99.0"));
        assert!(!newer("0.4.1", "0.4.1"));
        assert!(!newer("0.4.0", "0.4.1"));
        assert!(!newer("x", "0.4.1"), "写错的当 0");
    }

    #[test]
    fn reads_the_workspace_version() {
        assert_eq!(version_in(include_str!("../../Cargo.toml")).as_deref(), Some(current()));
        assert_eq!(version_in("[package]\nversion = \"1.0.0\"\n"), None);
    }
}
