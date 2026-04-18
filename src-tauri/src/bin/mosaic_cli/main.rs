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
    };
    mosaic_lib::ffmpeg::set_verbose(verbose);

    let code = match parsed.command {
        cli::Command::Screenshots(a)   => run::screenshots::run(a, &cfg).await,
        cli::Command::Sheet(a)         => run::sheet::run(a, &cfg).await,
        cli::Command::Reel(a)          => run::reel::run(a, &cfg).await,
        cli::Command::AnimatedSheet(a) => run::animated_sheet::run(a, &cfg).await,
        cli::Command::Probe(a)         => run::probe::run(a).await,
    };
    std::process::exit(code);
}
