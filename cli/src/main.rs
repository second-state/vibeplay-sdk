//! vibeplay：做掌机游戏的命令行。人和 agent 用的是同一套命令，`vibeplay help` 看全部。

mod device;
mod game;
mod market;
mod new;
mod pack;
mod player;
mod remix;
mod script;
mod source;
mod web;
mod window;

use anyhow::{bail, Context, Result};
use player::Player;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const USAGE_ZH: &str = "vibeplay：做掌机游戏（240×160、256 色、每秒 60 帧）

  vibeplay doctor                       查开发机缺什么（Rust、wasm32 目标、掌机连没连上）
  vibeplay new <目录> [--title 名字]     从模板起一个新游戏
  vibeplay run [游戏] [--lang zh] [--time 时间]
                                        开窗口玩，有声音。存档放在 <游戏>/save/
  vibeplay run [游戏] --headless --frames N [--input 脚本] [--data 目录] [--lang zh] [--time 时间]
                                        不开窗口，按脚本按键，印出每一个音调和录音
  vibeplay shot [游戏] --frames N[,N…] [--input 脚本] [--data 目录] [--out a.png] [--scale K] [--crop x,y,w,h] [--lang zh] [--time 时间]
                                        跑到第 N 帧，存成 PNG（默认放大 3 倍；真彩的不放大，640×480）。几个帧号就存几张：a-N.png
                                        看小精灵的细节：--crop 框住它，--scale 6 放大
  vibeplay web [游戏] [--out a.html]     出网页版：一个 HTML 文件，浏览器里就能玩，能发给别人
  vibeplay web --remote [--out a.html]  出不带游戏的网页版：打开时按 ?pkg=<地址> 拉一个 .vp 再玩（集市的网页用）
  vibeplay pack [游戏] [--out 目录]      打包成 <名字>.vp：拷进 TF 卡的「游戏」目录，或者 push
  vibeplay pair <掌机的地址>             和掌机配对（掌机要开开发者模式，屏幕上有口令）
  vibeplay push [游戏] [--watch]         打包、推到配好的掌机上、打开。--watch：一改就再推
  vibeplay login                        登录集市（GitHub 账号，要人在浏览器里点）
  vibeplay publish [游戏]                查清单、不开窗口跑 600 帧、截三张图、打包、发到集市，过了马上上架
  vibeplay publish [游戏] --source       同时公开源码：只收 Cargo.toml、Cargo.lock、app.toml、src/、assets/，先列出来，第一次要你答应
  vibeplay remix <名字> [目录]           改集市上公开了源码的游戏：下载最新那一版的源码，起一个新游戏（目录名是新名字，不写是 <名字>-remix），记下改自哪一版
  vibeplay status [名字]                 你在集市上的 app：每个版本上没上架、装了几次、最近没过的原因
  vibeplay unpublish <名字> [--version x.y.z]
                                        从集市下架。已经装了的掌机照常用
  vibeplay skill [install [--to 目录]]   印出给 agent 的说明；install 装进 Claude Code 和 Codex

集市的命令都能加 --market <地址>（本机试的时候指 wrangler dev），也能设环境变量 VIBEPLAY_MARKET。
--lang 是游戏看到的系统语言：en（默认）或 zh。掌机默认英文，人在设置里换。
--time 把钟定住，比如 2026-11-14T07:30:00（本机时区）或 2026-11-14T07:30:00+09:00。以后每 60 帧走一秒。
         看时间的 app（时钟、宠物）截图要稳就加它。不加用这台电脑的钟。
--data 是存档放在哪个目录。--headless 和 shot 不给就存不了：vibeplay::save 回 false，读也读不到。
[游戏] 是游戏的目录（有 app.toml），不写是当前目录。run、shot 也能直接给一个 .wasm 文件。
清单写了 frame = \"640x480\" 的是真彩 app（640×480、RGB565）：run 放在 720×480 的窗口中间，shot 存 640×480。网页版不认。
按键脚本一行一条：「帧 键…」或「起-止 键…」，帧从 0 数，# 后面是注释。
键：up down left right a b x y l1 r1 start select。
窗口的键：方向键，X=A，Z=B，S=X，A=Y，Q=L1，W=R1，回车=Start，右 Shift=Select，Esc 退出。
命令行的字跟着系统语言：系统是中文（LANG=zh_CN…）说中文，别的说英文。设 VIBEPLAY_LANG=zh 或 en 可以指定。";

const USAGE_EN: &str = "vibeplay: make games for the handheld (240×160, 256 colors, 60 frames a second)

  vibeplay doctor                       Check what this computer needs (Rust, the wasm32 target, the handheld connection)
  vibeplay new <dir> [--title name]     Start a new game from the template
  vibeplay run [game] [--lang zh] [--time time]
                                        Play in a window, with sound. Saves go in <game>/save/
  vibeplay run [game] --headless --frames N [--input script] [--data dir] [--lang zh] [--time time]
                                        Run with no window. Press keys from a script. Print each tone and each recording
  vibeplay shot [game] --frames N[,N…] [--input script] [--data dir] [--out a.png] [--scale K] [--crop x,y,w,h] [--lang zh] [--time time]
                                        Run to frame N and save it as a PNG (3 times larger by default; true-color apps stay 640×480). One file for each frame number: a-N.png
                                        To see the details of a sprite: put --crop around it and use --scale 6
  vibeplay web [game] [--out a.html]    Make the web version: one HTML file that plays in a browser. You can send it to other people
  vibeplay web --remote [--out a.html]  Make the web version with no game. When it opens, it gets a .vp from ?pkg=<address> (the Market web page uses it)
  vibeplay pack [game] [--out dir]      Pack the game into <name>.vp. Copy it into the games folder on the TF card, or push it
  vibeplay pair <handheld address>      Pair with the handheld (turn on developer mode; the code is on its screen)
  vibeplay push [game] [--watch]        Pack, send to the paired handheld, and open. --watch: send again after each change
  vibeplay login                        Log in to the Market (GitHub account; you click in a browser)
  vibeplay publish [game]               Check the manifest, run 600 frames with no window, take three screenshots, pack, and send to the Market. It is listed when it passes
  vibeplay publish [game] --source      Also make the source public: only Cargo.toml, Cargo.lock, app.toml, src/, assets/. It shows the list first and asks you the first time
  vibeplay remix <name> [dir]           Change a Market game that has public source: download the source of its latest version and start a new game (the folder name is the new name; default <name>-remix). It records the version it came from
  vibeplay status [name]                Your apps on the Market: for each version, if it is listed, how many installs, and why the last one did not pass
  vibeplay unpublish <name> [--version x.y.z]
                                        Remove from the Market. Handhelds that have it can still use it
  vibeplay skill [install [--to dir]]   Print the instructions for agents. install puts them into Claude Code and Codex

All Market commands accept --market <address> (for a local test, give the wrangler dev address). You can also set the VIBEPLAY_MARKET environment variable.
--lang is the system language that the game sees: en (default) or zh. The handheld is English by default; people change it in Settings.
--time stops the clock at a time, for example 2026-11-14T07:30:00 (local time zone) or 2026-11-14T07:30:00+09:00. After that, the clock moves one second every 60 frames.
         Use it for apps that show the time (clocks, pets) to get the same screenshots each time. Without it, the clock of this computer is used.
--data is the folder for saves. --headless and shot cannot save without it: vibeplay::save returns false, and a read gets nothing.
[game] is the game folder (it has app.toml). The default is the current folder. run and shot also accept a .wasm file.
A manifest with frame = \"640x480\" is a true-color app (640×480, RGB565): run shows it in the middle of a 720×480 window, and shot saves 640×480. The web version does not support it.
Key script: one line for each entry, \"frame keys…\" or \"start-end keys…\". Frames start at 0. Text after # is a comment.
Keys: up down left right a b x y l1 r1 start select.
Window keys: arrow keys, X=A, Z=B, S=X, A=Y, Q=L1, W=R1, Enter=Start, right Shift=Select, Esc quits.
The command line speaks Chinese when the system language is Chinese (LANG=zh_CN…), and English for other languages. VIBEPLAY_LANG=zh or VIBEPLAY_LANG=en sets it.";

/// 命令行说哪种话：先看 VIBEPLAY_LANG，再看 LC_ALL、LC_MESSAGES、LANG（取第一个不空的）。`zh` 开头说中文，别的说英文。
pub fn zh() -> bool {
    static ZH: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ZH.get_or_init(|| {
        ["VIBEPLAY_LANG", "LC_ALL", "LC_MESSAGES", "LANG"].iter().filter_map(|k| std::env::var(k).ok()).find(|v| !v.is_empty()).is_some_and(|v| v.to_lowercase().starts_with("zh"))
    })
}

/// 中文、英文各一句，照命令行的语言挑一句。两句都先拼好。
pub fn pick<T>(zh_: T, en: T) -> T {
    if zh() {
        zh_
    } else {
        en
    }
}

/// 集市请求带的 Accept-Language：服务端照它回中文或英文的错误。
pub fn lang() -> &'static str {
    pick("zh", "en")
}

/// 命令行的参数：位置参数和 `--名字 值`。
struct Args {
    pos: Vec<String>,
    opts: HashMap<String, String>,
}

/// 不带值的开关。
const SWITCHES: [&str; 4] = ["headless", "watch", "remote", "source"];

impl Args {
    fn parse(it: impl Iterator<Item = String>) -> Result<Args> {
        let mut a = Args { pos: Vec::new(), opts: HashMap::new() };
        let mut it = it.peekable();
        while let Some(x) = it.next() {
            if let Some(name) = x.strip_prefix("--") {
                let v = if SWITCHES.contains(&name) { String::new() } else { it.next().with_context(|| pick(format!("--{name} 后面要一个值"), format!("--{name} needs a value")))? };
                a.opts.insert(name.to_string(), v);
            } else {
                a.pos.push(x);
            }
        }
        Ok(a)
    }
    fn has(&self, k: &str) -> bool {
        self.opts.contains_key(k)
    }
    fn get(&self, k: &str) -> Option<&str> {
        self.opts.get(k).map(String::as_str)
    }
    fn path(&self, k: &str) -> Option<PathBuf> {
        self.get(k).map(PathBuf::from)
    }
    /// 第 `i` 个位置参数当游戏的目录，没有就是当前目录。
    fn game(&self, i: usize) -> PathBuf {
        PathBuf::from(self.pos.get(i).map(String::as_str).unwrap_or("."))
    }
    /// 只认这几个 `--名字`，别的报错：打错了不会悄悄被忽略。
    fn only(&self, known: &[&str]) -> Result<()> {
        for k in self.opts.keys() {
            if !known.contains(&k.as_str()) {
                bail!(pick(format!("这个命令没有 --{k}"), format!("This command has no --{k}")));
            }
        }
        Ok(())
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || matches!(args[0].as_str(), "-h" | "--help" | "help") {
        println!("{}", pick(USAGE_ZH, USAGE_EN));
        return;
    }
    if matches!(args[0].as_str(), "-V" | "--version" | "version") {
        println!("vibeplay {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if let Err(e) = run(args) {
        eprintln!("{}{e:#}", pick("vibeplay：", "vibeplay: "));
        std::process::exit(1);
    }
}

fn run(mut args: Vec<String>) -> Result<()> {
    let cmd = args.remove(0);
    let a = Args::parse(args.into_iter())?;
    match cmd.as_str() {
        "doctor" => doctor(),
        "new" => {
            a.only(&["title", "sdk"])?;
            let dir = a.pos.first().context(pick("要一个目录：vibeplay new mygame", "Give a folder: vibeplay new mygame"))?;
            new::create(Path::new(dir), a.get("title"), a.path("sdk").as_deref())
        }
        "run" if a.has("headless") => {
            a.only(&["headless", "frames", "input", "data", "lang", "time"])?;
            let frames: u64 = a.get("frames").context(pick("--headless 要 --frames N", "--headless needs --frames N"))?.parse().context(pick("--frames 要整数", "--frames needs an integer"))?;
            let mut p = load(&a, a.path("data"))?;
            p.run(frames, &script(&a)?, |n, t| match t.channel {
                // 第五个声道放的是短录音（tone_wav），没有频率。
                4 => println!("{}", pick(format!("帧 {n} 录音 毫秒={} 音量={}", t.ms, t.volume), format!("frame {n} recording ms={} volume={}", t.ms, t.volume))),
                _ => println!("{}", pick(format!("帧 {n} 音调 声道={} 频率={} 毫秒={} 音量={}", t.channel, t.hz, t.ms, t.volume), format!("frame {n} tone channel={} hz={} ms={} volume={}", t.channel, t.hz, t.ms, t.volume))),
            })?;
            println!("{}", pick(format!("跑完 {frames} 帧"), format!("Ran {frames} frames")));
            Ok(())
        }
        "run" => {
            a.only(&["data", "lang", "time"])?;
            let g = a.game(0);
            let data = a.path("data").or_else(|| g.is_dir().then(|| g.join("save")));
            let name = game::module_of(&g)?.1;
            let mut p = load(&a, data)?;
            window::play(&name, &mut p)
        }
        "shot" => {
            a.only(&["frames", "input", "data", "out", "scale", "crop", "lang", "time"])?;
            shot(&a)
        }
        "web" if a.has("remote") => {
            a.only(&["remote", "out"])?;
            let html = web::remote_page();
            let out = a.path("out").unwrap_or_else(|| PathBuf::from("dist/play.html"));
            write(&out, html.as_bytes())?;
            println!("{}", pick(format!("{}（打开时加 ?pkg=<包的地址>）", out.display()), format!("{} (add ?pkg=<package address> when you open it)", out.display())));
            Ok(())
        }
        "web" => {
            a.only(&["out"])?;
            let g = game::Game::open(&a.game(0))?;
            let html = web::page(&g.dir)?;
            let out = a.path("out").unwrap_or_else(|| PathBuf::from("dist").join(format!("{}.html", g.m.name)));
            write(&out, html.as_bytes())?;
            println!("{}", pick(format!("{}（{} KB，双击就能在浏览器里玩）", out.display(), html.len() / 1024), format!("{} ({} KB. Double-click it to play in a browser)", out.display(), html.len() / 1024)));
            Ok(())
        }
        "pack" => {
            a.only(&["out", "target"])?;
            let (bytes, g) = pack::pack(&a.game(0), Some(a.get("target").unwrap_or(pack::DEVICE_TARGET)))?;
            let out = a.path("out").unwrap_or_else(|| PathBuf::from("dist")).join(format!("{}.vp", g.m.name));
            write(&out, &bytes)?;
            println!("{}（{} KB）", out.display(), bytes.len() / 1024);
            Ok(())
        }
        "pair" => {
            a.only(&["code"])?;
            device::pair(a.pos.first().context(pick("要掌机的地址：vibeplay pair 192.168.1.23", "Give the handheld address: vibeplay pair 192.168.1.23"))?, a.get("code"))
        }
        "push" => {
            a.only(&["watch", "device"])?;
            device::push(&a.game(0), a.get("device"), a.has("watch"))
        }
        "login" => {
            a.only(&["market"])?;
            market::login(&market::url(a.get("market")))
        }
        "publish" => {
            a.only(&["market", "source"])?;
            market::publish(&a.game(0), &market::url(a.get("market")), a.has("source"))
        }
        "remix" => {
            a.only(&["market"])?;
            let name = a.pos.first().context(pick("要一个集市上的游戏名：vibeplay remix skyfire my-skyfire", "Give the name of a Market game: vibeplay remix skyfire my-skyfire"))?;
            remix::remix(name, a.pos.get(1).map(Path::new), &market::url(a.get("market")))
        }
        "status" => {
            a.only(&["market"])?;
            market::status(a.pos.first().map(String::as_str), &market::url(a.get("market")))
        }
        "unpublish" => {
            a.only(&["market", "version"])?;
            let name = a.pos.first().context(pick("要 app 的名字：vibeplay unpublish mygame", "Give the app name: vibeplay unpublish mygame"))?;
            market::unpublish(name, a.get("version"), &market::url(a.get("market")))
        }
        "skill" => {
            a.only(&["to"])?;
            match a.pos.first().map(String::as_str) {
                None => {
                    print!("{}", new::SKILL);
                    Ok(())
                }
                Some("install") => new::install_skill(a.path("to").as_deref()),
                Some(x) => bail!(pick(format!("skill 后面只认 install，不认 {x}"), format!("After skill, only install is accepted, not {x}"))),
            }
        }
        c => bail!(pick(format!("不认识的命令 {c}。vibeplay help 看全部命令"), format!("Unknown command {c}. Run vibeplay help to see all commands"))),
    }
}

fn script(a: &Args) -> Result<script::Script> {
    match a.path("input") {
        Some(p) => script::Script::parse(&std::fs::read_to_string(&p).with_context(|| pick(format!("读不了 {}", p.display()), format!("Cannot read {}", p.display())))?),
        None => Ok(script::Script::default()),
    }
}

/// 编好游戏、装进播放器。`data` 是存档的目录，None 时存不了。
fn load(a: &Args, data: Option<PathBuf>) -> Result<Player> {
    match a.get("lang").unwrap_or("en") {
        l @ ("en" | "zh") => qfull::full::set_lang(l),
        l => bail!(pick(format!("--lang 只认 en 和 zh，不认 {l}"), format!("--lang accepts only en and zh, not {l}"))),
    }
    let (module, _, memory_mb) = game::module_of(&a.game(0))?;
    if let Some(d) = &data {
        std::fs::create_dir_all(d).with_context(|| pick(format!("建不了 {}", d.display()), format!("Cannot make {}", d.display())))?;
    }
    let mut p = Player::load(&module, data, memory_mb)?;
    if let Some(t) = a.get("time") {
        let (start, tz) = parse_time(t)?;
        p.set_clock(start, tz);
    }
    Ok(p)
}

/// `--time`：`2026-11-14T07:30:00`（本机时区）或带偏移的 `2026-11-14T07:30:00+09:00`。中间的 T 也能写成空格。
fn parse_time(t: &str) -> Result<(i64, i32)> {
    use chrono::{Local, NaiveDateTime, TimeZone};
    let t = t.trim().replacen(' ', "T", 1);
    if let Ok(d) = chrono::DateTime::parse_from_rfc3339(&t) {
        return Ok((d.timestamp(), d.offset().local_minus_utc()));
    }
    let n = NaiveDateTime::parse_from_str(&t, "%Y-%m-%dT%H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(&t, "%Y-%m-%dT%H:%M"))
        .with_context(|| pick(format!("--time 看不懂「{t}」：写成 2026-11-14T07:30:00，或者加时区 +09:00"), format!("--time cannot read \"{t}\". Write 2026-11-14T07:30:00, or add a time zone: +09:00")))?;
    let d = Local.from_local_datetime(&n).earliest().with_context(|| pick(format!("本机时区里没有 {t} 这个时间"), format!("The time {t} does not exist in the local time zone")))?;
    Ok((d.timestamp(), d.offset().local_minus_utc()))
}

fn shot(a: &Args) -> Result<()> {
    let mut at: Vec<u64> = Vec::new();
    for s in a.get("frames").context(pick("shot 要 --frames N（几张就写 --frames 60,120）", "shot needs --frames N (for more than one: --frames 60,120)"))?.split(',') {
        at.push(s.trim().parse().with_context(|| pick(format!("--frames 里的 {s} 不是整数"), format!("{s} in --frames is not an integer")))?);
    }
    at.sort();
    let name = game::module_of(&a.game(0))?.1;
    let mut p = load(a, a.path("data"))?;
    // 真彩的画面（frame = "640x480"）默认不放大：和掌机的屏上一样大。
    let one = p.full().format() == qfull::full::Format::Rgb565;
    let scale: usize = a.get("scale").map(str::parse).transpose().context(pick("--scale 要整数", "--scale needs an integer"))?.unwrap_or(if one { 1 } else { 3 });
    let crop = match a.get("crop") {
        Some(c) => {
            let v: Vec<usize> = c.split(',').map(|x| x.trim().parse()).collect::<Result<_, _>>().context(pick("--crop 要 x,y,w,h 四个整数", "--crop needs four integers: x,y,w,h"))?;
            let [x, y, w, h] = v[..] else { bail!(pick("--crop 要 x,y,w,h 四个整数", "--crop needs four integers: x,y,w,h")) };
            Some((x, y, w, h))
        }
        None => None,
    };
    let s = script(a)?;
    for n in &at {
        p.run(n - p.frame_no, &s, |_, _| {})?;
        let out = match (a.path("out"), at.len()) {
            (Some(o), 1) => o,
            (Some(o), _) => o.with_file_name(format!("{}-{n}.png", o.file_stem().unwrap_or_default().to_string_lossy())),
            (None, _) => PathBuf::from(format!("{name}-{n}.png")),
        };
        if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        p.shot(&out, scale, crop)?;
        println!("{}", out.display());
    }
    Ok(())
}

fn write(out: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).with_context(|| pick(format!("建不了 {}", parent.display()), format!("Cannot make {}", parent.display())))?;
    }
    std::fs::write(out, bytes).with_context(|| pick(format!("写不了 {}", out.display()), format!("Cannot write {}", out.display())))
}

/// 查开发机：一样一样说好没好，没好的给出修的命令。
fn doctor() -> Result<()> {
    let mut bad = 0;
    let mut check = |ok: bool, what: &str, fix: &str| {
        if ok {
            println!("{}", pick(format!("  好  {what}"), format!("  ok       {what}")));
        } else {
            bad += 1;
            println!("{}", pick(format!("  缺  {what}：{fix}"), format!("  missing  {what}: {fix}")));
        }
    };
    let out = |cmd: &str, args: &[&str]| std::process::Command::new(cmd).args(args).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
    check(out("cargo", &["--version"]).is_some(), pick("Rust（cargo）", "Rust (cargo)"), pick("装 Rust：https://rustup.rs", "Install Rust: https://rustup.rs"));
    let targets = out("rustup", &["target", "list", "--installed"]).unwrap_or_default();
    check(targets.lines().any(|l| l.trim() == "wasm32-unknown-unknown"), pick("wasm32 目标", "wasm32 target"), "rustup target add wasm32-unknown-unknown");
    match device::saved() {
        Some(d) => check(
            device::reachable(&d),
            &pick(format!("掌机 {}", d.host), format!("handheld {}", d.host)),
            pick("掌机开着吗？和开发机在一个 Wi‑Fi 里吗？开发者模式开了吗？", "Is the handheld on? Is it on the same Wi‑Fi as this computer? Is developer mode on?"),
        ),
        None => println!("{}", pick("  --  掌机：还没配对（要推到掌机上再 vibeplay pair <地址>）", "  --       handheld: not paired (to push to a handheld, run vibeplay pair <address> first)")),
    }
    if bad == 0 {
        println!("{}", pick("都好了。vibeplay new mygame 起一个游戏。", "All good. Run vibeplay new mygame to start a game."));
    }
    Ok(())
}
