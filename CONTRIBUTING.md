# Contributing to memtop

Thanks for your interest in memtop! Bug reports, ideas and pull requests are welcome.
Every change is reviewed by the maintainer before it is merged, so don't worry about getting
everything perfect on the first try.

## Before you start

- **Bugs:** open an [issue](https://github.com/SalomonAvila/memtop/issues/new/choose) with
  your distribution, terminal, `memtop -V` and steps to reproduce.
- **Features or larger changes:** open an issue first so we can agree on the approach
  before you spend time on it.
- **Security problems:** do not open a public issue; see [SECURITY.md](SECURITY.md).

## Development setup

You need Rust 1.95 or newer (installed with [rustup](https://rustup.rs)) and a C linker
(see the [README](README.md#from-source)).

```sh
git clone https://github.com/<your-user>/memtop.git
cd memtop
cargo run --release
```

## Workflow

memtop uses [Git Flow](https://nvie.com/posts/a-successful-git-branching-model/):

- `main` holds published releases only; nobody pushes to it directly.
- `develop` is where changes are integrated. **Open your pull request against `develop`.**

1. Fork the repository and create a branch from `develop`:
   ```sh
   git switch develop
   git switch -c feature/short-description   # or fix/short-description
   ```
2. Make your change in small, focused commits.
3. Run the same checks as CI:
   ```sh
   cargo fmt --check
   cargo clippy --all-targets --locked -- -D warnings
   cargo test --locked
   ```
4. Push the branch and open a pull request against `develop`, filling in the template.

CI runs on every pull request. For contributors from outside the project, the maintainer
approves each CI run before it starts.

## Guidelines

- **Commits** follow [Conventional Commits](https://www.conventionalcommits.org/) in English,
  imperative and lowercase: `feat: add a disk I/O chart`, `fix(gpu): handle missing hwmon`.
  Mark breaking changes with `!` and a `BREAKING CHANGE:` footer.
- **Code, comments and documentation** are written in English.
- **User-visible text** goes through `src/i18n.rs`: add a field to `Texts` and give it a value
  in both `ES` and `EN` (the compiler enforces it). Never hardcode interface text.
- **Configuration keys and CLI flags** are part of the public interface. Changing or removing
  one is a breaking change and needs to be discussed in an issue first. See the README's
  [compatibility policy](README.md#compatibility-policy) for the scope of semantic-versioning
  guarantees.
- **Changelog:** add an entry under `Unreleased` in [CHANGELOG.md](CHANGELOG.md) for any
  change users will notice.
- **Tests:** add or update tests for logic you change (parsing, formatting, config…).
- **Dependencies:** avoid adding new crates unless they are clearly needed; memtop ships as
  a single static binary with no system dependencies.

## Review

Every pull request is reviewed by the maintainer, who may ask for changes. Pushing new
commits after an approval requires a new review. Once approved and green, the maintainer
merges it into `develop`, and it ships in the next release.

## License

By contributing, you agree that your contributions are licensed under the
[MIT License](LICENSE), the same license as the project.

Please follow the [Code of Conduct](CODE_OF_CONDUCT.md) in every interaction.
