//! Captures what every run's `metadata.json` needs to say where its numbers
//! came from: the source revision, whether the tree was clean, the compiler,
//! and the target. Nothing here touches the physics; a build without `git`
//! records "unknown" and the run proceeds.

use std::process::Command;

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn main() {
    let commit = run("git", &["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let dirty = run("git", &["status", "--porcelain", "--untracked-files=no"])
        .map(|s| if s.is_empty() { "false" } else { "true" })
        .unwrap_or("unknown");
    let rustc = std::env::var("RUSTC")
        .ok()
        .and_then(|rc| run(&rc, &["--version"]))
        .unwrap_or_else(|| "unknown".into());
    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".into());
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "unknown".into());

    println!("cargo:rustc-env=UNIVERSE_GIT_COMMIT={commit}");
    println!("cargo:rustc-env=UNIVERSE_GIT_DIRTY={dirty}");
    println!("cargo:rustc-env=UNIVERSE_RUSTC={rustc}");
    println!("cargo:rustc-env=UNIVERSE_TARGET={target}");
    println!("cargo:rustc-env=UNIVERSE_PROFILE={profile}");
    // Rebuild when the checked-out commit changes, so the recorded revision
    // cannot go stale between a commit and the next run.
    println!("cargo:rerun-if-changed=.git/HEAD");
    if let Some(head) = run("git", &["symbolic-ref", "-q", "HEAD"]) {
        println!("cargo:rerun-if-changed=.git/{head}");
    }
    println!("cargo:rerun-if-changed=.git/index");
}
