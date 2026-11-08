# Maintainer notes

How memtop is released and kept building. Contributors don't need any of this; see
[CONTRIBUTING.md](../CONTRIBUTING.md) instead.

## Branches and merging

memtop uses Git Flow:

| Branch | Purpose | Merges into |
|---|---|---|
| `feature/*`, `fix/*` | One change each, branched from `develop` | `develop` |
| `develop` | Integration; default branch | — |
| `release/X.Y.Z` | Version bump and release notes, branched from `develop` | `main`, then back into `develop` |
| `hotfix/X.Y.Z` | Urgent fix, branched from `main` | `main`, then back into `develop` |
| `main` | Published releases only, each one tagged `vX.Y.Z` | — |

Pull requests are merged with a merge commit (`--merge`, never squash or rebase) so the
history keeps each branch.

`develop` and `main` are protected: they need the required checks to pass, an approving
review from the code owner, approval of the latest push and every conversation resolved.
GitHub does not let authors approve their own pull requests, so the maintainer merges their
own pull requests as admin:

```sh
gh pr checks <number> --watch
gh pr merge <number> --merge --admin
```

Don't use `gh pr merge --delete-branch`: it fails when the working tree has uncommitted
changes, and it would delete a `release/*` branch before it is merged back into `develop`.
Delete branches afterwards:

```sh
git branch -d <branch> && git push origin --delete <branch>
```

Pull requests from outside contributors only run CI after the maintainer approves the run
(Settings → Actions → "Require approval for all external contributors").

## Releasing a version

1. Create the release branch from an up-to-date `develop`:
   ```sh
   git switch develop && git pull --ff-only
   git switch -c release/X.Y.Z
   ```
2. Bump the version:
   - `version` in `Cargo.toml`, then `cargo build` to update `Cargo.lock`.
   - `Version:` in `packaging/memtop.spec`, plus a new entry at the top of `%changelog`
     (date format: `Sun Nov 08 2026`, from `date '+%a %b %d %Y'` with `LC_TIME=C`).
   - In `CHANGELOG.md`, move the `Unreleased` entries under `## [X.Y.Z] - YYYY-MM-DD` and
     update the comparison links at the bottom.
3. Run the checks and commit:
   ```sh
   cargo fmt --check && cargo clippy --all-targets --locked -- -D warnings && cargo test --locked
   git add Cargo.toml Cargo.lock packaging/memtop.spec CHANGELOG.md
   git commit -m 'chore(release): publish version X.Y.Z'
   ```
4. Push and open the pull request against `main`; merge it once CI is green:
   ```sh
   git push -u origin release/X.Y.Z
   gh pr create --base main --title 'chore(release): publish version X.Y.Z' --body 'See CHANGELOG.md.'
   gh pr checks <number> --watch && gh pr merge <number> --merge --admin
   ```
   Merging into `main` triggers the COPR build automatically (see below).
5. Tag the release on `main`. The `release.yml` workflow checks that the tag matches
   `Cargo.toml`, creates the GitHub release and attaches the static binaries:
   ```sh
   git switch main && git pull --ff-only
   git tag vX.Y.Z && git push origin vX.Y.Z
   gh run watch "$(gh run list --workflow release.yml --limit 1 --json databaseId --jq '.[0].databaseId')"
   ```
6. Merge the release back into `develop` and clean up:
   ```sh
   gh pr create --base develop --head release/X.Y.Z --title 'chore: merge release X.Y.Z back into develop' --body 'Git Flow back-merge.'
   gh pr checks <number> --watch && gh pr merge <number> --merge --admin
   git switch develop && git pull --ff-only
   git branch -d release/X.Y.Z && git push origin --delete release/X.Y.Z
   ```
7. Check the COPR build and the installed package:
   ```sh
   copr-cli list-builds mrmich/memtop | head -3
   sudo dnf upgrade --refresh memtop && memtop -V
   ```

Write commit messages, pull request titles and bodies in single quotes: in interactive bash,
a `!` inside double quotes (as in `feat!:`) triggers history expansion.

## COPR

The Fedora packages live in the [`mrmich/memtop`](https://copr.fedorainfracloud.org/coprs/mrmich/memtop/)
COPR project. The `memtop` package is built from this repository (`make_srpm` method,
`.copr/Makefile` and `packaging/memtop.spec`, branch `main`), and a GitHub webhook rebuilds
it on every push to `main`.

To build a spec change locally the same way COPR does:

```sh
make -f .copr/Makefile srpm outdir=$PWD/rpm spec=$PWD/packaging/memtop.spec
rpmbuild --rebuild rpm/memtop-*.src.rpm
```

`make srpm` packages the last commit (`git archive HEAD`), not uncommitted changes.

### When a new Fedora release comes out

COPR does **not** add new releases to the project by itself. Until it is done, users who
upgrade Fedora silently stop getting memtop (the repository is configured with
`skip_if_unavailable=True`). For Fedora N:

1. Enable its chroots. `--chroot` replaces the whole list, so pass every chroot to keep:
   ```sh
   copr-cli modify mrmich/memtop \
     --chroot fedora-<N-2>-x86_64 --chroot fedora-<N-2>-aarch64 \
     --chroot fedora-<N-1>-x86_64 --chroot fedora-<N-1>-aarch64 \
     --chroot fedora-<N>-x86_64 --chroot fedora-<N>-aarch64 \
     --chroot fedora-rawhide-x86_64 --chroot fedora-rawhide-aarch64
   copr-cli list mrmich | grep -E 'fedora-[0-9a-z]+-' | sort
   ```
   Drop the chroots of Fedora releases that reached end of life.
2. Rebuild the current release for it:
   ```sh
   copr-cli build-package mrmich/memtop --name memtop -r fedora-<N>-x86_64 -r fedora-<N>-aarch64
   ```
3. Add `"<N>"` to the `fedora` matrix of the `rpm` job in `.github/workflows/ci.yml` (and
   remove end-of-life releases), through a normal pull request.
4. Update the required checks on `develop` and `main` so they match the new matrix (see
   [Required checks](#required-checks)).
5. Update the list of supported releases in the README (Installation → Fedora).

## Required checks

The required checks are matched by job name, so renaming a job in `ci.yml` or changing the
Fedora matrix requires updating the branch protection, or pull requests wait forever for a
check that no longer exists. To set them (keep the reviews part as is):

```sh
for b in develop main; do
  echo '{"required_status_checks":{"strict":true,"checks":[{"context":"Format, lint and tests","app_id":15368},{"context":"RPM (Fedora 43)","app_id":15368},{"context":"RPM (Fedora 44)","app_id":15368},{"context":"RPM (Fedora 45)","app_id":15368}]},"enforce_admins":false,"required_pull_request_reviews":{"dismiss_stale_reviews":true,"require_code_owner_reviews":true,"required_approving_review_count":1,"require_last_push_approval":true},"restrictions":null,"required_conversation_resolution":true,"allow_force_pushes":false,"allow_deletions":false}' \
    | gh api -X PUT "repos/SalomonAvila/memtop/branches/$b/protection" --input - --jq '[.required_status_checks.checks[].context]'
done
```

The GPU database (`data/pci-gpu.ids`) is refreshed with `scripts/update-pci-ids.sh`; see the
[README](../README.md#updating-the-gpu-database).
