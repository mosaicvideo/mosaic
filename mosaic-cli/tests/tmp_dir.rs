use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn bin(root: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("mosaic-cli").unwrap();
    cmd.env("MOSAIC_CLI_CONFIG", root.path().join("config.toml"));
    cmd
}

#[test]
fn tmp_dir_routes_all_renderers_and_embedded_font() {
    let root = TempDir::new().unwrap();
    let scratch_name = "Miki's ☆ [x],; 50% scratch";
    let scratch = root.path().join(scratch_name);
    fs::create_dir(&scratch).unwrap();
    let scratch = scratch.canonicalize().unwrap();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../src-tauri/tests/fixtures/sample.mp4");
    let sample = root.path().join("Miki's ☆ [x],; 50% %{pts}.mp4");
    fs::copy(fixture, &sample).unwrap();

    for (command, args) in [
        ("screenshots", vec!["--count", "1"]),
        ("sheet", vec!["--cols", "1", "--rows", "1"]),
        ("reel", vec!["--count", "1", "--clip-length", "1"]),
        (
            "animated-sheet",
            vec!["--cols", "2", "--rows", "1", "--clip-length", "1"],
        ),
    ] {
        let out = root.path().join(command);
        fs::create_dir(&out).unwrap();
        let result = bin(&root)
            .current_dir(root.path())
            .arg(command)
            .args(args)
            // Relative punctuation/Unicode path exercises filters as well as argv.
            .args(["--tmp-dir", scratch_name, "--verbose", "-o"])
            .arg(&out)
            .arg(&sample)
            .assert()
            .success();
        let stderr = String::from_utf8_lossy(&result.get_output().stderr);
        let scratch_prefix = format!("{}{}", scratch.display(), std::path::MAIN_SEPARATOR);
        let font_prefix = mosaic_lib::drawtext::font_for_ffmpeg(std::path::Path::new(&scratch_prefix));
        let render_lines: Vec<_> = stderr
            .lines()
            .filter(|line| {
                line.starts_with("+ ") && line.contains("-hide_banner -loglevel error -y")
            })
            .collect();
        assert!(!render_lines.is_empty(), "{stderr}");
        assert!(
            render_lines
                .iter()
                .all(|line| line.contains(&scratch_prefix)),
            "{stderr}"
        );
        assert!(fs::read_dir(&out)
            .unwrap()
            .any(|e| fs::metadata(e.unwrap().path()).unwrap().len() > 0));
        if command == "sheet" || command == "animated-sheet" {
            assert!(
                stderr.contains(&format!("fontfile={font_prefix}")),
                "{stderr}"
            );
            // Static font storage survives process exit; verify real extracted bytes,
            // not only a path printed in an argument. The outer TempDir owns cleanup.
            assert!(fs::read_dir(&scratch).unwrap().any(|entry| {
                let path = entry.unwrap().path();
                path.is_file()
                    && fs::read(path).unwrap()
                        == include_bytes!("../../src-tauri/assets/fonts/DejaVuSans.ttf")
            }));
        }
        // Media working directories are cleaned after publication.
        assert!(fs::read_dir(&scratch)
            .unwrap()
            .all(|e| !e.unwrap().path().is_dir()));
    }
}

#[test]
fn tmp_dir_rejects_missing_directory_and_regular_file_before_config() {
    let root = TempDir::new().unwrap();
    let file = root.path().join("file");
    fs::write(&file, b"not a directory").unwrap();
    for path in [root.path().join("missing"), file] {
        bin(&root)
            .arg("--tmp-dir")
            .arg(&path)
            .args(["probe", "unused.mp4"])
            .assert()
            .code(2)
            .stderr(predicate::str::contains("invalid --tmp-dir"))
            .stderr(predicate::str::contains(path.to_string_lossy().as_ref()));
    }
    assert!(!root.path().join("config.toml").exists());
}

#[cfg(unix)]
#[test]
fn tmp_dir_rejects_unwritable_directory() {
    use std::os::unix::fs::PermissionsExt;
    let root = TempDir::new().unwrap();
    let scratch = root.path().join("readonly");
    fs::create_dir(&scratch).unwrap();
    fs::set_permissions(&scratch, fs::Permissions::from_mode(0o500)).unwrap();
    // Privileged runners may legitimately bypass permissions; report that limitation.
    if tempfile::NamedTempFile::new_in(&scratch).is_ok() {
        eprintln!("skipping permission assertion: runner can write to mode 0500 directory");
    } else {
        bin(&root)
            .arg("--tmp-dir")
            .arg(&scratch)
            .args(["probe", "unused.mp4"])
            .assert()
            .code(2)
            .stderr(predicate::str::contains("invalid --tmp-dir"));
        assert!(!root.path().join("config.toml").exists());
    }
    fs::set_permissions(&scratch, fs::Permissions::from_mode(0o700)).unwrap();
}

#[test]
fn tmp_dir_does_not_interfere_with_self_contained_commands() {
    let root = TempDir::new().unwrap();
    for args in [vec!["--help"], vec!["completions", "bash"], vec!["manpage"]] {
        bin(&root)
            .env("PATH", "")
            .arg("--tmp-dir")
            .arg(root.path().join("missing"))
            .args(args)
            .assert()
            .success()
            .stdout(predicate::str::contains("tmp"));
    }
    assert!(!root.path().join("config.toml").exists());
}
