//! Files the Mac changed under bind mounts, raised as inotify events in the
//! VM. Apple's virtiofs server tells the guest nothing when a file changes
//! on the Mac, so file watchers in containers (Vite, webpack, nodemon,
//! `cargo watch`) never heard of a save. The daemon watches the bind sources
//! with FSEvents and sends `OP\tPATH` lines (`Msg::FsEvents`); the guest
//! kernel's /proc/kiri-fsnotify (guest/kernel/patches/0006) raises the
//! event for each, as a change made in the VM would. Nothing is written to
//! the files, so nothing goes back to the Mac.

use std::io::Write;

/// Where the Mac's filesystem is mounted in the VM (mod.rs).
const MAC: &str = "/mnt/mac/";
const PROC: &str = "/proc/kiri-fsnotify";

/// Hands the kernel the lines that name a file under the Mac's mount; a
/// kernel without the interface (or a host-process engine) drops them.
pub fn raise<'a>(lines: impl Iterator<Item = &'a str>) {
    let batch = batch(lines);
    if batch.is_empty() {
        return;
    }
    let Ok(mut f) = std::fs::OpenOptions::new().write(true).open(PROC) else { return };
    // One write per line: the kernel answers each with its own result, and a
    // file gone again by now (ENOENT) must not drop the rest.
    for line in batch.split_inclusive('\n') {
        let _ = f.write_all(line.as_bytes());
    }
}

/// The lines the kernel takes: `C`, `M`, `A` or `D`, a tab, and a path under
/// /mnt/mac with no `..` and no newline.
fn batch<'a>(lines: impl Iterator<Item = &'a str>) -> String {
    let mut out = String::new();
    for line in lines {
        let Some((op, path)) = line.split_once('\t') else { continue };
        let ok = matches!(op, "C" | "M" | "A" | "D")
            && path.starts_with(MAC)
            && path.len() > MAC.len()
            && !path.contains('\n')
            && !path.split('/').any(|part| part == "..");
        if ok {
            out.push_str(op);
            out.push('\t');
            out.push_str(path);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::batch;

    #[test]
    fn only_paths_under_the_macs_mount() {
        let lines = ["M\t/mnt/mac/Users/a/app/main.js", "D\t/etc/passwd", "C\t/mnt/mac/../etc/x", "X\t/mnt/mac/Users/a", "M\t/mnt/mac/", "nonsense"];
        assert_eq!(batch(lines.into_iter()), "M\t/mnt/mac/Users/a/app/main.js\n");
    }
}
