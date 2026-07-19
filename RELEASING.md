# Releasing OpenWRaw

Standard procedure for cutting an OpenWRaw release. This repo ships one
crate and one Python package from a single tag:

| Artifact | Kind | Published to |
| --- | --- | --- |
| `openwraw` | Rust crate (`cargo publish`) | crates.io |
| `openwraw` | maturin wheels + sdist (`crates/openwraw-py`) | PyPI |

The version lives in exactly one place, `[workspace.package] version` in
the root `Cargo.toml`. Both crates inherit it via `version.workspace =
true`, and the root `pyproject.toml`'s `dynamic = ["version"]` is resolved
by maturin from the same workspace manifest at build time - there is
nothing else to bump in lockstep.

## Steps

1. **Bump the version.** Edit `[workspace.package] version` in `Cargo.toml`.
   Run `cargo build` (or `cargo check`) once so `Cargo.lock` picks up the
   new `openwraw`/`openwraw-py` versions.
   - The version must not already exist on crates.io or PyPI - publishes
     are irreversible, you cannot overwrite or re-upload a version.

2. **Update `CHANGELOG.md`.** Move the `[Unreleased]` entries under a new
   `## [X.Y.Z] - YYYY-MM-DD` heading (see existing entries for the level of
   detail expected - this project links back to the GitHub issues each
   change closes).

3. **Commit.** `git commit -m "release: vX.Y.Z"` (see `git log` for
   precedent - a short one-line summary of what the release bundles is
   fine when it's not obvious from the version bump alone).

4. **Confirm CI and audit are green before tagging.**

   ```sh
   ./scripts/check-release-ready.sh
   ```

   This checks the most recent `ci.yml` and `audit.yml` runs for `HEAD`
   (pass a different ref/SHA to check something other than `HEAD`) and
   exits non-zero if either hasn't run or isn't green. `publish.yml`
   triggers directly on `push: tags: ["v*"]` with no dependency on CI or
   audit passing - GitHub Actions has no way for one workflow file to
   `needs:` a job defined in another workflow file, so this has to be
   checked by hand before tagging rather than enforced inside
   `publish.yml` itself. See
   [#14](https://github.com/Sigilweaver/OpenWRaw/issues/14).

   Do not tag if the script fails. Either wait for CI/audit to finish and
   re-run it, or fix whatever's red first.

5. **Tag and push.**

   ```sh
   git tag -a vX.Y.Z -m "vX.Y.Z"
   git push origin main
   git push origin vX.Y.Z
   ```

   Pushing the tag triggers `publish.yml`: `cargo publish` (best-effort,
   `continue-on-error` so an already-published crate or a flaky registry
   doesn't block the PyPI side), then wheel builds across all five
   OS/target legs plus an sdist build, then `pypi-publish` (`needs:
   [build-wheels, build-sdist]`).

6. **Watch the run.** `gh run watch` or check the Actions tab. If a wheel
   leg fails on a transient runner issue, `pypi-publish` will be skipped
   (it needs all of `build-wheels`); re-run just the failed job with
   `gh run rerun <run-id> --failed` rather than re-tagging.

7. **Verify.**
   - crates.io: `cargo info openwraw` (or check
     `https://crates.io/crates/openwraw/versions`) shows the new version.
   - PyPI: `pip index versions openwraw` (or check
     `https://pypi.org/project/openwraw/#history`) shows the new version.

8. **Update the ops repo's `versions.toml`** entry for
   `mass_spectrometry.openwraw` (`crates_version` / `pypi_version`) if this
   repo is tracked there.
