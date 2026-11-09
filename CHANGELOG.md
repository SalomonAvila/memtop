# Changelog

All notable changes to memtop are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- The terminal is shown as "ptyxis" instead of "ptyxis-agent" when memtop runs in Ptyxis.

## [0.2.0] - 2026-11-08

### Changed

- **Breaking:** command-line flags are now in English:

  | 0.1.x | Now |
  |---|---|
  | `--intervalo` | `--interval` (`-i` is unchanged) |
  | `--idioma` | `--lang` |
  | `--config-ejemplo` | `--print-config` |

- **Breaking:** configuration keys and values are now in English. Regenerate your file with
  `memtop --print-config` or rename the keys by hand:

  | 0.1.x | Now |
  |---|---|
  | `idioma` | `language` |
  | `intervalo` | `interval` |
  | `pestana_inicial` | `start_tab` |
  | `"resumen"`, `"almacenamiento"`, `"red"`, `"procesos"` | `"summary"`, `"storage"`, `"network"`, `"processes"` |
  | `[procesos]` `orden` / `solo_mios` | `[processes]` `sort` / `mine_only` |
  | `"memoria"`, `"nombre"`, `"usuario"` | `"memory"`, `"name"`, `"user"` |
  | `[colores]` `acento` / `tenue` / `etiqueta` / `seleccion` | `[colors]` `accent` / `muted` / `label` / `selection` |

- `config.ejemplo.toml` is now `config.example.toml`.
- `--help` and every error message printed before the interface opens are translated. They
  follow `LANG`, then `--lang`, then the configuration file, and default to English.
- `--lang` also applies to `--help` when given after it.
- The left column shows only the System panel by default.
- The English tab names are now "Summary" and "RAM" (were "Overview" and "Memory"), matching
  the `start_tab` values.
- The documentation is now in English.

### Added

- `logo` configuration key to show the memtop logo above the System panel.
- Fedora 45 packages in COPR, and CI builds and installs the RPM on Fedora 45.

### Removed

- **Breaking:** the Pokémon panel, the `f` key, the `--sin-pokemon` flag and the `pokemon`
  configuration key. memtop no longer runs `pokemon-colorscripts`.

## [0.1.1] - 2026-11-07

### Added

- CI builds and installs the RPM on Fedora 43 and 44.

## [0.1.0] - 2026-11-02

### Added

- First release: system summary and live monitor for CPU, RAM, storage, GPU, network and
  processes in seven tabs.
- Optional random Pokémon in the left panel when `pokemon-colorscripts` is installed.
- Interface in English and Spanish, detected from the environment.
- Configuration file in `$XDG_CONFIG_HOME/memtop/config.toml`.
- Static binaries for `x86_64` and `aarch64`, and an RPM package for Fedora via COPR.

[Unreleased]: https://github.com/SalomonAvila/memtop/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/SalomonAvila/memtop/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/SalomonAvila/memtop/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/SalomonAvila/memtop/releases/tag/v0.1.0
