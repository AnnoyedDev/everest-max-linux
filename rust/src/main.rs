#[macro_use]
mod i18n;
mod actions;
mod apply;
mod audio;
mod config;
mod daemon;
mod device;
mod fx;
mod gui;
mod icons;
mod images;
mod input;
mod keys;
mod obs;
mod openrgb;
mod pad;
mod protocol;
mod setup;
mod stats;
mod theme;
mod uinput;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = match args.first().map(String::as_str) {
        None | Some("gui") => gui::run().map_err(|e| e.to_string()),
        Some("daemon") => {
            daemon::run();
            Ok(())
        }
        Some("apply") => {
            let cfg = config::Config::load();
            device::Device::open().and_then(|mut d| {
                apply::lighting(&mut d, &cfg, true)?;
                apply::all_bindings(&mut d, &cfg)
            })
        }
        Some("setup") => {
            let mut cfg = config::Config::load();
            let errors = setup::fix_all(&mut cfg);
            cfg.save().ok();
            for c in setup::run(&cfg) {
                println!("{} {} — {}", if c.ok { "✔" } else { "✖" }, c.title, c.detail);
            }
            if errors.is_empty() { Ok(()) } else { Err(errors.join("\n")) }
        }
        Some("version") => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("install-service") => setup::install_service(),
        Some("export-icons") => {
            let dir = args.get(1).map(std::path::PathBuf::from).unwrap_or_else(setup::icons_dir);
            images::export_library(&dir).map(|n| println!("{n} icônes dans {}", dir.display()))
        }
        Some("clock") => device::Device::open().and_then(|mut d| apply::clock_now(&mut d)),
        Some("image") if args.len() >= 3 => {
            let cfg = config::Config::load();
            let src = &args[2];
            let mut prog = |x: f32| eprint!("\r{:.0} %", x * 100.0);
            device::Device::open().and_then(|mut d| match args[1].as_str() {
                "dock" => apply::dock_image(&mut d, src, &mut prog),
                k @ ("d1" | "d2" | "d3" | "d4") => {
                    let mut dk = config::DisplayKey::default();
                    if std::path::Path::new(src).is_file() {
                        dk.image = src.clone();
                    } else if icons::svg(src).is_some() {
                        dk.icon = src.clone();
                    } else {
                        dk.text = src.clone();
                    }
                    apply::display_key(&mut d, cfg.profile, k.as_bytes()[1] - b'1', &dk, &mut prog)
                }
                _ => Err("cible : d1, d2, d3, d4 ou dock".into()),
            })
        }
        _ => Err("usage : everest [gui | daemon | setup | version | apply | install-service | export-icons [dossier] | clock | image <d1-d4|dock> <fichier|icône|texte>]".into()),
    };
    if let Err(e) = r {
        eprintln!("erreur : {e}");
        std::process::exit(1);
    }
}
