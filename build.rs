//! Build script: emit the `web_embedded` cfg when the built web UI is present.
//!
//! `web/dist/` is a build artifact — gitignored, and absent in a crates.io
//! tarball (`cargo publish` ships git-tracked files only). When present at
//! compile time (local builds, CI, the release binary job), the React SPA is
//! baked into the binary via `include_dir!` (see `src/embedded_web.rs`) so the
//! daemon serves it with **no first-run download**. When absent
//! (`cargo install` from crates.io), the cfg is not set and the daemon falls
//! back to downloading `web-dist.zip` from GitHub Releases at runtime.
//!
//! This mirrors `src/default_skills.rs` embedding `share/default-skills/`,
//! except default-skills is git-tracked (always present) while `web/dist/` is
//! generated — hence the presence check rather than unconditional embedding.
//!
//! `--all-features` safety: a fresh clone has no `web/dist/`, so the cfg stays
//! off and `include_dir!` never expands. No hard compile error.

fn main() {
    let marker = std::path::Path::new("web/dist/index.html");
    if marker.is_file() {
        println!("cargo:rustc-cfg=web_embedded");
    }
    // Re-run when the dist is (re)built or removed, so a `bun run build`
    // after a clean compile picks up without a manual `cargo clean`.
    // Declare the custom cfg so rustc's `unexpected_cfgs` lint accepts it
    // (emitted above when web/dist exists; absent otherwise).
    println!("cargo::rustc-check-cfg=cfg(web_embedded)");
}
