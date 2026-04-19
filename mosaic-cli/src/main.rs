// src-tauri/src/bin/mosaic_cli/main.rs

mod cli;
mod config;
mod font;
mod hints;
mod progress;
mod run;
mod signals;

use clap::Parser;

#[tokio::main]
async fn main() {
    let parsed = cli::Cli::parse();

    // Self-contained subcommands that don't need config or ffmpeg/ffprobe/mediainfo.
    match &parsed.command {
        cli::Command::Completions(a) => {
            use clap::CommandFactory;
            let mut cmd = cli::Cli::command();
            let bin_name = cmd.get_name().to_string();
            clap_complete::generate(a.shell, &mut cmd, bin_name, &mut std::io::stdout());
            return;
        }
        cli::Command::Manpage => {
            use clap::CommandFactory;
            let cmd = cli::Cli::command();
            if let Err(e) = clap_mangen::Man::new(cmd).render(&mut std::io::stdout()) {
                eprintln!("manpage render failed: {e}");
                std::process::exit(1);
            }
            return;
        }
        _ => {}
    }

    let cfg = match config::resolve_path() {
        Some((p, is_explicit)) => match config::load_or_create(&p, is_explicit) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(2);
            }
        },
        None => config::Config::default(),
    };
    if let Err(e) = cfg.validate() {
        eprintln!("{e}");
        std::process::exit(2);
    }

    let verbose = match &parsed.command {
        cli::Command::Screenshots(a)   => a.shared.verbose,
        cli::Command::Sheet(a)         => a.shared.verbose,
        cli::Command::Reel(a)          => a.shared.verbose,
        cli::Command::AnimatedSheet(a) => a.shared.verbose,
        cli::Command::Probe(_)         => false,
        cli::Command::Completions(_) | cli::Command::Manpage => unreachable!("handled above"),
    };
    mosaic_lib::ffmpeg::set_verbose(verbose);

    let code = match parsed.command {
        cli::Command::Screenshots(a)   => run::screenshots::run(a, &cfg).await,
        cli::Command::Sheet(a)         => run::sheet::run(a, &cfg).await,
        cli::Command::Reel(a)          => run::reel::run(a, &cfg).await,
        cli::Command::AnimatedSheet(a) => run::animated_sheet::run(a, &cfg).await,
        cli::Command::Probe(a)         => run::probe::run(a).await,
        cli::Command::Completions(_) | cli::Command::Manpage => unreachable!("handled above"),
    };
    std::process::exit(code);
}
