# Roadmap to 1.0

memtop 1.0 is not about adding features. It is a promise: once 1.0 is out, the command-line
flags, the configuration file and the exit codes stay compatible for the whole 1.x series,
and breaking them needs a 2.0. This document lists what has to happen before making that
promise.

Progress is tracked in the [milestones](https://github.com/SalomonAvila/memtop/milestones):
each item below has an issue. Items marked **help wanted** are good places to start
contributing (see [CONTRIBUTING.md](../CONTRIBUTING.md)).

## Where things stand (0.2.0)

- Works on Fedora 43, 44, 45 and Rawhide (COPR) and on any distribution (static binaries for
  `x86_64` and `aarch64`).
- Every tab and the help dialog were exercised at terminal sizes from 10×3 to 200×60 without
  crashes.
- 38 tests cover configuration, CLI parsing, language detection, PCI and nvidia-smi parsing
  and formatting. The interface state (`app.rs`) and drawing (`ui.rs`) have no tests yet.
- Tested on real hardware only on an Intel + NVIDIA laptop.

## 0.3 — Freeze the public interface

What users type must be final before 1.0.

- **Decide how unknown configuration keys are handled.** Today an unknown key is an error,
  which catches typos. The downside: a config that uses a key added in, say, 1.3 makes 1.1
  refuse to start, which hurts people who share dotfiles across machines. Options:
  1. Keep the error (strict, simple; newer configs don't work on older versions).
  2. Turn it into a warning shown at startup and keep running.
  3. Keep the error for keys that never existed (typos) and only warn for keys known to
     belong to a newer version.

  This is the hardest decision to change after 1.0, so it comes first.
- **Final review of names.** One last pass over every flag (`--interval`, `--lang`,
  `--config`, `--print-config`), configuration key and value, and exit code (`0`, `1`, `2`).
  Anything renamed after 1.0 is a breaking change.
- **Write down the compatibility policy.** State in the README what semantic versioning
  covers (flags, configuration keys and values, exit codes) and what it does not (screen
  layout, colors, interface text, the order of rows).
- **Tests for the interface state** (`app.rs`): sorting and reversing, search, the "mine only"
  filter, and that the selection follows the same PID when the list is reordered.
  **help wanted**
- **Rendering tests** (`ui.rs`) with ratatui's `TestBackend`: draw every tab and dialog at
  several sizes (including tiny ones) and check that nothing panics, so the manual check
  becomes automatic. **help wanted**

## 0.4 — Hardening and distribution

- **Check the minimum Rust version in CI.** `Cargo.toml` declares `rust-version = "1.95"`,
  but CI only builds with stable. Add a job that builds and tests with 1.95.
- **Audit dependencies in CI** with `cargo-deny` (security advisories and licenses).
- **Dependabot version updates** for Cargo and GitHub Actions. Security updates are already
  enabled; every Dependabot pull request still goes through review.
- **Shell completions** for bash, zsh and fish, shipped in the RPM and in the release
  archives. **help wanted**
- **Man page** (`memtop.1`), shipped the same way. **help wanted**
- **Publish on crates.io** so `cargo install memtop` works. The name is free as of
  2026-11-08. Limit the published files with `include` in `Cargo.toml` (today the package
  would also carry `.github/`, `.copr/`, `packaging/` and `docs/`), and add the publish step
  to [docs/RELEASING.md](RELEASING.md).

## 1.0.0-rc.1 — Real-world testing

No new features. Publish a release candidate and ask for testing on hardware the project has
not seen yet: AMD GPUs, Intel-only machines, desktops without a battery, virtual machines and
containers, and other distributions using the static binary. The GPU tab depends the most on
the hardware. Fix what comes up, in more release candidates if needed. **help wanted**

## 1.0.0

The last release candidate, unchanged or with its fixes, and the compatibility policy in
effect.

## Not blocking 1.0

These can all land in 1.x without breaking anything, so 1.0 does not wait for them:

- Process tree view.
- Mouse support.
- Configurable key bindings.
- More interface languages.
- Inclusion in the official Fedora repositories (requires packaging every crate separately
  instead of bundling them).

Ideas are welcome as [feature requests](https://github.com/SalomonAvila/memtop/issues/new/choose).
