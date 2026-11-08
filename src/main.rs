//! memtop: system information and resource monitor in a single terminal screen.
//!
//! Layout:
//! - [`system`]: reading CPU, memory, disks, network, processes, sensors and battery.
//! - [`gpu`] and [`pci`]: GPU detection and their marketing names.
//! - [`logo`]: memtop logo in pixel art for the left panel.
//! - [`config`]: configuration file.
//! - [`i18n`]: interface language and translated text.
//! - [`app`]: interface state and key handling.
//! - [`ui`]: drawing of every tab with ratatui.

mod app;
mod config;
mod gpu;
mod i18n;
mod logo;
mod pci;
mod system;
mod ui;

use std::{
    env,
    io::{IsTerminal, stdout},
    path::PathBuf,
    process::ExitCode,
    time::{Duration, Instant},
};

use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;

use app::App;
use config::Config;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Command-line options. `None` = not given, so the configuration applies.
#[derive(Debug, Default, PartialEq)]
struct Options {
    config: Option<PathBuf>,
    interval: Option<f64>,
    language: Option<i18n::LanguageSetting>,
}

impl Options {
    /// Applies the options given on the command line on top of the configuration.
    fn apply(self, mut c: Config) -> Config {
        if let Some(i) = self.interval {
            c.interval = i;
        }
        if let Some(i) = self.language {
            c.language = i;
        }
        c
    }
}

/// What to do after parsing the options.
#[derive(Debug, PartialEq)]
enum Action {
    Run,
    Help,
    Version,
    PrintConfig,
}

/// Parses the options. `--help`, `--version` and `--print-config` are resolved at the end
/// so a `--lang` in any position changes the language of the help.
fn parse_args(args: impl IntoIterator<Item = String>) -> Result<(Action, Options), String> {
    let t = i18n::t();
    let mut o = Options::default();
    let mut action = None;
    let mut args = args.into_iter();
    while let Some(a) = args.next() {
        match a.as_str() {
            "-h" | "--help" => action = action.or(Some(Action::Help)),
            "-V" | "--version" => action = action.or(Some(Action::Version)),
            "--print-config" => action = action.or(Some(Action::PrintConfig)),
            "-c" | "--config" => {
                o.config = Some(PathBuf::from(args.next().ok_or(t.cli_missing_path)?))
            }
            "--lang" => {
                let v = args.next().ok_or(t.cli_missing_lang)?;
                let i = v
                    .parse()
                    .map_err(|_| format!("{}: {v:?}", t.cli_invalid_lang))?;
                o.language = Some(i);
            }
            "-i" | "--interval" => {
                let s: f64 = args
                    .next()
                    .and_then(|v| v.replace(',', ".").parse().ok())
                    .filter(|v: &f64| v.is_finite() && *v >= config::MIN_INTERVAL)
                    .ok_or(t.cli_invalid_interval)?;
                o.interval = Some(s);
            }
            other => return Err(format!("{}: {other}", t.cli_unknown_option)),
        }
    }
    Ok((action.unwrap_or(Action::Run), o))
}

fn main() -> ExitCode {
    // Until the options are parsed, messages use the environment's language.
    let (action, options) = match parse_args(env::args().skip(1)) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("memtop: {e}\n{}", i18n::t().cli_try_help);
            return ExitCode::from(2);
        }
    };
    // From here on, the help and the configuration file errors honor `--lang`.
    if let Some(i) = options.language {
        i18n::set(i.resolve());
    }
    match action {
        Action::Run => {}
        Action::Help => {
            print!("{}", i18n::t().cli_help);
            return ExitCode::SUCCESS;
        }
        Action::Version => {
            println!("memtop {VERSION}");
            return ExitCode::SUCCESS;
        }
        Action::PrintConfig => {
            print!("{}", config::EXAMPLE);
            return ExitCode::SUCCESS;
        }
    }

    let config = match config::load(options.config.as_deref()) {
        Ok(c) => options.apply(c),
        Err(e) => {
            eprintln!("memtop: {e:#}");
            return ExitCode::from(2);
        }
    };

    // Final language: configuration file and, on top of it, `--lang`.
    i18n::set(config.language.resolve());

    if !stdout().is_terminal() {
        eprintln!("memtop: {}", i18n::t().cli_no_terminal);
        return ExitCode::FAILURE;
    }

    match start(config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("memtop: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn start(config: Config) -> Result<()> {
    ui::set_theme(config.colors.clone());
    // First reading before entering the interface: CPU usage needs two samples.
    let monitor = system::Monitor::new()?;

    // `ratatui::init` enables raw mode and the alternate screen, and installs a hook that
    // restores the terminal if the program panics.
    let mut terminal = ratatui::init();
    let app = App::new(monitor, &config);
    let result = run(&mut terminal, app, config.refresh_interval());
    ratatui::restore();
    result
}

/// Main loop: draws, waits for keys until the next tick and refreshes the data.
fn run(terminal: &mut DefaultTerminal, mut app: App, interval: Duration) -> Result<()> {
    let mut next_tick = Instant::now() + interval;

    while !app.quit {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        let wait = next_tick.saturating_duration_since(Instant::now());
        if event::poll(wait)? {
            match event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => app.key(k),
                _ => {}
            }
        }
        if Instant::now() >= next_tick {
            app.tick();
            next_tick = Instant::now() + interval;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Action, String> {
        parse_args(args.iter().map(|s| s.to_string())).map(|(a, _)| a)
    }

    fn options(args: &[&str]) -> Options {
        match parse_args(args.iter().map(|s| s.to_string())) {
            Ok((Action::Run, o)) => o,
            other => panic!("should run: {other:?}"),
        }
    }

    #[test]
    fn no_arguments_keep_the_configuration() {
        assert_eq!(options(&[]), Options::default());
        assert_eq!(options(&[]).apply(Config::default()), Config::default());
    }

    #[test]
    fn parses_every_option() {
        let o = options(&["--lang", "en", "-i", "2,5", "-c", "/tmp/c.toml"]);
        assert_eq!(o.config, Some(PathBuf::from("/tmp/c.toml")));
        let c = o.apply(Config::default());
        assert_eq!(c.language, i18n::LanguageSetting::En);
        assert_eq!(c.refresh_interval(), Duration::from_millis(2500));
    }

    #[test]
    fn options_take_precedence_over_the_file() {
        let file =
            config::parse("language = \"es\"\ninterval = 5.0\nstart_tab = \"network\"").unwrap();
        let c = options(&["-i", "2", "--lang", "en"]).apply(file);
        assert_eq!(c.interval, 2.0);
        assert_eq!(c.language, i18n::LanguageSetting::En);
        // Whatever was not given on the command line comes from the file.
        assert_eq!(c.start_tab, app::Tab::Network);
    }

    #[test]
    fn rejects_the_old_spanish_options() {
        for old in [
            "--intervalo",
            "--idioma",
            "--config-ejemplo",
            "--sin-pokemon",
        ] {
            assert!(parse(&[old, "1"]).is_err(), "{old}");
        }
    }

    #[test]
    fn actions_that_do_not_open_the_interface() {
        assert_eq!(parse(&["--help"]), Ok(Action::Help));
        assert_eq!(parse(&["-V"]), Ok(Action::Version));
        assert_eq!(parse(&["--print-config"]), Ok(Action::PrintConfig));
        // --lang is kept even after --help; the first action wins.
        let (action, o) = parse_args(["--help", "--lang", "es"].map(String::from)).unwrap();
        assert_eq!(action, Action::Help);
        assert_eq!(o.language, Some(i18n::LanguageSetting::Es));
        assert_eq!(parse(&["-V", "--help"]), Ok(Action::Version));
        // An error in another option is reported even with --help.
        assert!(parse(&["--help", "--lang", "fr"]).is_err());
    }

    #[test]
    fn rejects_invalid_values() {
        assert!(parse(&["--config"]).is_err());
        assert!(parse(&["--lang"]).is_err());
        assert!(parse(&["--lang", "fr"]).is_err());
        assert!(parse(&["-i", "0.1"]).is_err());
        assert!(parse(&["-i", "nan"]).is_err());
        assert!(parse(&["--unknown"]).is_err());
    }
}
