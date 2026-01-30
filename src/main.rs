use std::fs::OpenOptions;
use std::io::{self, Write};
use std::process;
use std::sync::Arc;

use giallo::Registry;
use log;

mod cli;
mod commands;
mod config;
mod fifo;
mod highlight;
mod highlighting;
mod kakoune;
mod registry_loader;
mod server;
mod server_resources;

use cli::{parse_args, Mode};
use commands::{list_grammars, list_themes};
use config::Config;
use registry_loader::{load_custom_grammars, load_custom_themes};
use server::run_server;
use server_resources::ServerResources;

fn main() {
    let (mode, verbose, _plain) = parse_args();
    let base_dir = std::env::temp_dir().join(format!("giallo-kak-{}", process::id()));

    if let Mode::KakouneRc = mode {
        const RC: &str = include_str!("../rc/giallo.kak");
        println!("{RC}");
        return;
    }

    if verbose {
        simple_logger::init_with_level(log::Level::Debug).expect("failed to initialize logging");
    }

    log::info!("starting giallo-kak server");
    log::debug!("base_dir: {}", base_dir.display());

    let resources = ServerResources::new(base_dir.clone());

    if let Err(e) = resources.setup_signal_handler() {
        log::warn!("failed to setup signal handler: {}", e);
    } else {
        log::debug!("signal handler installed successfully");
    }

    let mut registry = match Registry::builtin() {
        Ok(registry) => registry,
        Err(err) => {
            log::error!("failed to load giallo registry: {err}");
            eprintln!("failed to load giallo registry: {err}");
            process::exit(1);
        }
    };
    log::debug!("registry loaded successfully");

    let config = Config::load();
    log::debug!("config loaded: {:?}", config);

    if let Some(ref grammars_path) = config.grammars_path {
        if let Err(err) = load_custom_grammars(&mut registry, grammars_path) {
            log::error!("failed to load custom grammars: {err}");
            eprintln!("warning: failed to load custom grammars: {err}");
        }
    }

    if let Some(ref themes_path) = config.themes_path {
        if let Err(err) = load_custom_themes(&mut registry, themes_path) {
            log::error!("failed to load custom themes: {err}");
            eprintln!("warning: failed to load custom themes: {err}");
        }
    }

    registry.link_grammars();
    log::debug!("grammars linked");

    let registry = Arc::new(registry);
    log::debug!("registry wrapped in Arc for thread sharing");

    match mode {
        Mode::Stdio => {
            log::debug!("running in stdio mode");
            let stdin = io::stdin();
            let stdout = io::stdout();
            let mut stdin_lock = stdin.lock();
            let mut stdout_lock = stdout.lock();
            if let Err(err) = run_server(
                &mut stdin_lock,
                &mut stdout_lock,
                Arc::clone(&registry),
                &config,
                false,
                Some(&base_dir),
                &resources,
            ) {
                log::error!("server error: {err}");
                eprintln!("server error: {err}");
            }
        }
        Mode::Oneshoot => {
            log::debug!("running in oneshot mode");
            let stdin = io::stdin();
            let stdout = io::stdout();
            let mut stdin_lock = stdin.lock();
            let mut stdout_lock = stdout.lock();
            if let Err(err) = run_server(
                &mut stdin_lock,
                &mut stdout_lock,
                Arc::clone(&registry),
                &config,
                true,
                Some(&base_dir),
                &resources,
            ) {
                log::error!("oneshot error: {err}");
                eprintln!("oneshot error: {err}");
            }
        }
        Mode::Fifo { req, resp } => {
            log::debug!("running in fifo mode");
            log::debug!("req fifo: {req}");
            if let Some(ref r) = resp {
                log::debug!("resp fifo: {r}");
            }

            let req_file = match OpenOptions::new().read(true).write(true).open(&req) {
                Ok(file) => file,
                Err(err) => {
                    log::error!("failed to open fifo for read: {req}: {err}");
                    eprintln!("failed to open fifo for read: {req}: {err}");
                    process::exit(1);
                }
            };

            let mut req_reader = io::BufReader::new(req_file);

            let mut resp_writer: Box<dyn Write> = if let Some(resp_path) = resp {
                match OpenOptions::new().write(true).open(&resp_path) {
                    Ok(file) => Box::new(file),
                    Err(err) => {
                        log::error!("failed to open fifo for write: {resp_path}: {err}");
                        eprintln!("failed to open fifo for write: {resp_path}: {err}");
                        process::exit(1);
                    }
                }
            } else {
                Box::new(io::stdout())
            };

            if let Err(err) = run_server(
                &mut req_reader,
                &mut resp_writer,
                Arc::clone(&registry),
                &config,
                false,
                Some(&base_dir),
                &resources,
            ) {
                log::error!("fifo server error: {err}");
                eprintln!("fifo server error: {err}");
            }
        }
        Mode::ListGrammars => {
            list_grammars(&registry, &config, false);
        }
        Mode::ListGrammarsPlain => {
            list_grammars(&registry, &config, true);
        }
        Mode::ListThemes => {
            list_themes(&registry, &config, false);
        }
        Mode::ListThemesPlain => {
            list_themes(&registry, &config, true);
        }
        Mode::KakouneRc => unreachable!(),
    }

    log::info!("giallo-kak server exiting");
}
