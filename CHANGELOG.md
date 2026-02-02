# Changelog

## 0.4.0

- Add crates.io publish job to release workflow
- Switch CLI parsing from manual loop to clap derive
- Enable `--version` flag via clap
- Add HTTP timeouts and retry logic
- Add structured exit codes
- Add Unix signal forwarding to child process
- Exit on healthcheck HTTP failure instead of silently continuing
- Modernize release workflow and CI
- Simplify environment variable filtering
- Cap stored output to 40k bytes in both code paths

## 0.3.0

- Bump edition to 2024 and update dependencies
- Cap stored output to 40k bytes
- Add nix flake
- Create helper struct `TeeCursor` for cleaner tee implementation
- Add tests for UUID parsing
- Standardize on using `EXIT_CODE` constant
- Add `trim_trailing` function with tests
- Fix spacing on help message

## 0.2.0

- Rename project to `hcp` to avoid crates.io name clash
- Fix bug where `HCP_IGNORE_CODE` was accidentally passed to subprocess
- Fix bug in handling programs that use `\r`
- Bump ureq dependency for nightly build fix
- Update CI to use `v`-prefixed tags

## 0.1.5

- Refactor for cleaner code structure
- Rename pipe function and add comments
- Simplify function signatures

## 0.1.4

- Add static compilation on Windows
- Fix static compile on Windows

## 0.1.3

- Release with minor fixes

## 0.1.2

- Release with minor fixes

## 0.1.1

- Initial CI and release pipeline setup
- Add version to binary

## 0.1.0

- Initial release
