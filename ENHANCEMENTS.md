# Enhancement Suggestions for hcp

This document contains suggested enhancements for the `hcp` project, organized by priority.

## HIGH PRIORITY - Quick Wins

### 1. Add Clippy to CI Pipeline
**Files:** `.github/workflows/ci.yml`
**Description:** Add clippy linting to catch potential bugs and code smells.

**Action Items:**
- [ ] Add clippy job to CI workflow
- [ ] Configure clippy to deny warnings (`-D warnings`)
- [ ] Fix any clippy warnings that appear

**Suggested CI addition:**
```yaml
clippy:
  name: clippy
  runs-on: ubuntu-latest
  steps:
    - name: Checkout repository
      uses: actions/checkout@v4
    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        override: true
        profile: minimal
        components: clippy
    - name: Run clippy
      run: cargo clippy -- -D warnings
```

---

### 2. Add Timeout Support for HTTP Requests
**Files:** `src/main.rs` (lines 182-199)
**Description:** HTTP calls can hang indefinitely if network is slow/unresponsive.

**Action Items:**
- [ ] Configure ureq with timeout (default 30 seconds)
- [ ] Make timeout configurable via environment variable or flag
- [ ] Document timeout behavior

**Code change:**
```rust
let agent = ureq::AgentBuilder::new()
    .timeout(std::time::Duration::from_secs(30))
    .build();
```

---

### 3. Upgrade Deprecated GitHub Actions
**Files:** `.github/workflows/ci.yml`, `.github/workflows/release.yml`
**Description:** Update from `actions/checkout@v2` to `@v4`.

**Action Items:**
- [ ] Update checkout action to v4 in ci.yml
- [ ] Update checkout action to v4 in release.yml
- [ ] Update actions-rs/toolchain if newer alternatives exist

---

### 4. Update `bstr` Dependency
**Files:** `Cargo.toml:15`
**Description:** Upgrade from 0.2 to 1.x for better performance and API improvements.

**Action Items:**
- [ ] Update `bstr = "0.2"` to `bstr = "1.0"`
- [ ] Test compatibility (API may have changed)
- [ ] Update any affected code
- [ ] Run full test suite

---

## MEDIUM PRIORITY - Code Quality & Maintainability

### 5. Extract Magic Numbers to Constants
**Files:** `src/main.rs:54`
**Description:** Replace hardcoded 16KB buffer size with named constant.

**Action Items:**
- [ ] Add `const TEE_BUFFER_SIZE: usize = 16 * 1024;`
- [ ] Use constant in tee function
- [ ] Document why this size was chosen

---

### 6. Improve Error Context
**Files:** `src/main.rs:325-342`
**Description:** Provide better error messages for thread join failures.

**Action Items:**
- [ ] Enhance error messages for stdout thread failures
- [ ] Enhance error messages for stderr thread failures
- [ ] Add context about what was being read when error occurred

---

### 7. Add Documentation Comments
**Files:** `src/main.rs` (multiple locations)
**Description:** Add rustdoc comments for public interfaces and complex functions.

**Action Items:**
- [ ] Add rustdoc for `TeeCursor` struct (lines 20-47)
- [ ] Add rustdoc for `tee()` function (lines 49-83)
- [ ] Add rustdoc for `trim_trailing()` (lines 11-18)
- [ ] Add rustdoc for `Uuid` and `HealthCheck` types (lines 117-202)
- [ ] Run `cargo doc` to verify formatting

---

### 8. Modularize Code
**Files:** `src/main.rs`
**Description:** Split single 405-line file into logical modules.

**Action Items:**
- [ ] Create `src/healthcheck.rs` for HealthCheck and Uuid types
- [ ] Create `src/tee.rs` for TeeCursor and tee function
- [ ] Create `src/lib.rs` for shared utilities
- [ ] Keep `src/main.rs` focused on CLI parsing and orchestration
- [ ] Update tests to work with new structure

**Suggested structure:**
```
src/
├── main.rs          # CLI parsing, orchestration
├── healthcheck.rs   # HealthCheck and Uuid types
├── tee.rs          # TeeCursor and tee function
└── lib.rs          # Common utilities (trim_trailing, etc.)
```

---

### 9. Add Help Flag Support
**Files:** `src/main.rs:240-250`
**Description:** Allow users to see help with `--help` or `-h` flag.

**Action Items:**
- [ ] Add `--help` and `-h` flag handling
- [ ] Exit with code 0 after showing help
- [ ] Update help text if needed

---

### 10. Extract Environment Variable Names to Constants
**Files:** `src/main.rs:233-239`
**Description:** Use constants instead of hardcoded strings for environment variables.

**Action Items:**
- [ ] Define constants for `HCP_ID`, `HCP_TEE`, `HCP_IGNORE_CODE`
- [ ] Replace all hardcoded strings with constants
- [ ] Makes future refactoring easier and prevents typos

---

## LOWER PRIORITY - Feature Enhancements

### 11. Add Version Flag
**Files:** `src/main.rs:231-378`
**Description:** Allow users to check version with `--version` or `-v`.

**Action Items:**
- [ ] Add `--version` and `-v` flag handling
- [ ] Use `env!("CARGO_PKG_VERSION")` to get version
- [ ] Exit with code 0 after showing version

---

### 12. Support Custom Healthcheck URLs
**Files:** `src/main.rs:159-179`
**Description:** Support self-hosted healthchecks instances via custom base URL.

**Action Items:**
- [ ] Add `--hcp-url` flag and `HCP_URL` env variable
- [ ] Modify `HealthCheck` to accept custom base URL
- [ ] Default to `https://hc-ping.com` if not specified
- [ ] Document in README

---

### 13. Add Retry Logic for Network Failures
**Files:** `src/main.rs:182-199`
**Description:** Retry transient network errors with exponential backoff.

**Action Items:**
- [ ] Implement retry logic for `/start` endpoint
- [ ] Implement retry logic for `/finish` and `/fail` endpoints
- [ ] Use exponential backoff (e.g., 1s, 2s, 4s)
- [ ] Make max retries configurable
- [ ] Log retry attempts

---

### 14. Add Integration Tests
**Files:** New `tests/` directory
**Description:** Add integration tests for end-to-end functionality.

**Action Items:**
- [ ] Create `tests/integration_test.rs`
- [ ] Test command execution and capture
- [ ] Test exit code handling
- [ ] Test `--hcp-tee` flag behavior
- [ ] Test `--hcp-ignore-code` flag behavior
- [ ] Mock healthchecks.io API for testing

---

### 15. Add Output Size Limits
**Files:** `src/main.rs:356-366`
**Description:** Truncate very large outputs to avoid overwhelming healthchecks API.

**Action Items:**
- [ ] Define `MAX_OUTPUT_SIZE` constant (e.g., 100KB)
- [ ] Truncate stdout if exceeds limit
- [ ] Truncate stderr if exceeds limit
- [ ] Show truncation message with original size
- [ ] Make limit configurable

---

### 16. Add Signal Handling
**Files:** `src/main.rs:323-377`
**Description:** Forward SIGTERM/SIGINT to child process for graceful shutdown.

**Action Items:**
- [ ] Add `ctrlc` or `signal-hook` dependency
- [ ] Forward signals to child process
- [ ] Handle cleanup on signal receipt
- [ ] Test signal handling behavior

---

### 17. Add Verbose/Quiet Logging Modes
**Files:** `src/main.rs` (multiple locations)
**Description:** Control logging verbosity with flags.

**Action Items:**
- [ ] Add `--hcp-quiet` flag to suppress all output
- [ ] Add `--hcp-verbose` flag for detailed logging
- [ ] Control `eprintln!` output based on verbosity
- [ ] Add environment variable equivalents

---

## DOCUMENTATION ENHANCEMENTS

### 18. Expand README.md
**Files:** `README.md`
**Description:** Add more examples, use cases, and troubleshooting.

**Action Items:**
- [ ] Add cron job example
- [ ] Add environment variable examples
- [ ] Show example output
- [ ] Add troubleshooting section
- [ ] Add FAQ section
- [ ] Add badges (build status, version, etc.)

---

### 19. Add CHANGELOG.md
**Files:** New `CHANGELOG.md`
**Description:** Track changes between versions.

**Action Items:**
- [ ] Create CHANGELOG.md following Keep a Changelog format
- [ ] Document changes in 0.2.0
- [ ] Document changes in 0.1.x
- [ ] Update with each release

---

### 20. Add Examples Directory
**Files:** New `examples/` directory
**Description:** Provide example scripts showing common usage patterns.

**Action Items:**
- [ ] Create `examples/` directory
- [ ] Add cron job example
- [ ] Add Docker container example
- [ ] Add systemd service example
- [ ] Add backup script example

---

## CI/CD IMPROVEMENTS

### 21. Add Code Coverage
**Files:** `.github/workflows/ci.yml`
**Description:** Track test coverage using tarpaulin or llvm-cov.

**Action Items:**
- [ ] Add coverage job to CI
- [ ] Use `cargo-tarpaulin` or `cargo-llvm-cov`
- [ ] Upload coverage to codecov.io or coveralls
- [ ] Add coverage badge to README

---

### 22. Add Dependabot
**Files:** New `.github/dependabot.yml`
**Description:** Automatically update dependencies.

**Action Items:**
- [ ] Create `.github/dependabot.yml`
- [ ] Configure for Cargo dependencies
- [ ] Configure for GitHub Actions
- [ ] Set weekly update schedule

**Config:**
```yaml
version: 2
updates:
  - package-ecosystem: "cargo"
    directory: "/"
    schedule:
      interval: "weekly"
  - package-ecosystem: "github-actions"
    directory: "/"
    schedule:
      interval: "weekly"
```

---

### 23. Update CI to Use Modern Ubuntu
**Files:** `.github/workflows/ci.yml:38,104`
**Description:** Ubuntu 18.04 is EOL, update to ubuntu-latest.

**Action Items:**
- [ ] Replace `ubuntu-18.04` with `ubuntu-latest`
- [ ] Update package installation if needed
- [ ] Test CI passes on new Ubuntu version

---

## TESTING ENHANCEMENTS

### 24. Add Error Path Tests
**Files:** `src/main.rs` (test module)
**Description:** Test error handling paths.

**Action Items:**
- [ ] Test invalid UUID format handling
- [ ] Test missing command handling
- [ ] Test network error handling
- [ ] Test child process failure handling
- [ ] Test environment variable parsing

---

### 25. Add Property-Based Tests
**Files:** New test module
**Description:** Use proptest for randomized testing.

**Action Items:**
- [ ] Add `proptest` dependency
- [ ] Add property tests for UUID validation
- [ ] Add property tests for tee function
- [ ] Add property tests for trim_trailing

---

## PERFORMANCE (Minor Optimizations)

### 26. Consider Using BufReader
**Files:** `src/main.rs:306-320`
**Description:** Potentially more efficient for very large outputs.

**Action Items:**
- [ ] Benchmark current implementation
- [ ] Test with BufReader
- [ ] Compare performance
- [ ] Implement if meaningful improvement

---

### 27. Reuse HTTP Agent
**Files:** `src/main.rs:182-199`
**Description:** Single agent instance could reduce overhead.

**Action Items:**
- [ ] Create agent once and pass to HealthCheck
- [ ] Benchmark to verify improvement
- [ ] Consider if added complexity is worth it

---

## Implementation Priority Recommendation

**Phase 1 (Immediate):**
- Items 1-4 (High priority quick wins)
- Items 9, 11 (Help/version flags - user-facing)

**Phase 2 (Next sprint):**
- Items 7, 10 (Documentation and constants)
- Items 18, 19 (Documentation files)
- Items 22, 23 (CI improvements)

**Phase 3 (Future):**
- Item 8 (Modularization - if project grows)
- Items 12-17 (Feature enhancements based on user feedback)
- Items 21, 24-25 (Testing improvements)

---

## Notes

- This is a well-structured, production-ready codebase
- Suggestions are enhancements, not critical fixes
- Prioritize based on user feedback and actual needs
- Some items (like modularization) may not be needed unless the project grows significantly
