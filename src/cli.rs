use std::process;

pub enum Mode {
    Stdio,
    Oneshoot,
    Fifo { req: String, resp: Option<String> },
    KakouneRc,
    ListGrammars,
    ListGrammarsPlain,
    ListThemes,
    ListThemesPlain,
}

pub fn print_help() {
    let commit = option_env!("GIT_COMMIT").unwrap_or("unknown");
    println!(
        "giallo-kak {} ({}) - Kakoune syntax highlighter using TextMate grammars",
        env!("CARGO_PKG_VERSION"),
        commit
    );
    println!();
    println!("USAGE:");
    println!("  giallo-kak [OPTIONS] [COMMAND]");
    println!();
    println!("OPTIONS:");
    println!("  -h, --help              Print this help message");
    println!("  -v, --verbose           Enable verbose logging");
    println!("      --version           Print version information");
    println!("      --oneshot           Run once and exit (for testing)");
    println!("      --fifo <PATH>       Use FIFO at PATH for IPC");
    println!("      --resp <PATH>       Response FIFO path");
    println!();
    println!("COMMANDS:");
    println!("  init                    Print Kakoune integration script");
    println!("  list-grammars           List available grammar files");
    println!("  list-themes             List available theme files");
    println!();
    println!("GRAMMAR/THEME LIST OPTIONS:");
    println!("  --plain                 Output plain list (one per line, for fzf)");
    println!();
    println!("EXAMPLES:");
    println!("  giallo-kak init                    # Print Kakoune script");
    println!("  giallo-kak list-grammars           # List grammars with descriptions");
    println!("  giallo-kak list-grammars --plain   # List grammar names only");
    println!("  giallo-kak list-themes --plain | fzf  # Interactive theme selection");
    println!();
    println!("For more information: https://github.com/yukai/giallo.kak");
}

pub fn parse_args() -> (Mode, bool, bool) {
    let mut oneshot = false;
    let mut fifo_req: Option<String> = None;
    let mut fifo_resp: Option<String> = None;
    let mut kakoune_rc = false;
    let mut verbose = false;
    let mut list_grammars = false;
    let mut list_themes = false;
    let mut plain_output = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                process::exit(0);
            }
            "--version" => {
                let commit = option_env!("GIT_COMMIT").unwrap_or("unknown");
                println!("giallo-kak {} ({})", env!("CARGO_PKG_VERSION"), commit);
                process::exit(0);
            }
            "--verbose" | "-v" => verbose = true,
            "--oneshot" => oneshot = true,
            "init" | "--kakoune" | "--print-rc" => kakoune_rc = true,
            "list-grammars" | "--list-grammars" => list_grammars = true,
            "list-themes" | "--list-themes" => list_themes = true,
            "--plain" => plain_output = true,
            "--fifo" => {
                if let Some(path) = args.next() {
                    fifo_req = Some(path);
                }
            }
            "--resp" => {
                if let Some(path) = args.next() {
                    fifo_resp = Some(path);
                }
            }
            _ => {}
        }
    }

    let mode = if list_grammars {
        if plain_output {
            Mode::ListGrammarsPlain
        } else {
            Mode::ListGrammars
        }
    } else if list_themes {
        if plain_output {
            Mode::ListThemesPlain
        } else {
            Mode::ListThemes
        }
    } else if let Some(req) = fifo_req {
        Mode::Fifo {
            req,
            resp: fifo_resp,
        }
    } else if kakoune_rc {
        Mode::KakouneRc
    } else if oneshot {
        Mode::Oneshoot
    } else {
        Mode::Stdio
    };

    (mode, verbose, plain_output)
}
