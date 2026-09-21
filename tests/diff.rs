mod common;

use common::{Repo, snapshot};
use std::fs;

#[test]
fn combines_changes_without_touching_the_repository() {
    let repo = Repo::new();
    repo.write("mixed", "original\n");
    repo.write("deleted", "gone\n");
    repo.write("cancelled", "original\n");
    repo.write(".gitignore", "ignored\n");
    repo.commit();
    repo.write("mixed", "staged\n");
    repo.write("cancelled", "staged\n");
    repo.write("added", "new staged file\n");
    repo.git(&["add", "."]);
    repo.write("mixed", "final\n");
    repo.write("cancelled", "original\n");
    fs::remove_file(repo.0.join("deleted")).unwrap();
    repo.write("untracked", "new untracked file\n");
    repo.write("ignored", "hidden\n");
    let before = snapshot(&repo.0);

    let output = repo.spek(&[]).stdout;
    assert_eq!(output, repo.spek(&["diff"]).stdout);
    assert_eq!(snapshot(&repo.0), before);
    let text = String::from_utf8(output).unwrap();
    assert_eq!(
        text,
        "A  added     | 1 +\nMM cancelled | 0\n D deleted   | 1 -\nMM mixed     | 2 +-\n?? untracked | 1 +\n 5 files changed, 3 insertions, 2 deletions\n"
    );
}

#[test]
fn works_before_first_commit_and_with_empty_files() {
    let repo = Repo::new();
    assert!(repo.spek(&[]).stdout.is_empty());
    repo.write("staged", "first\n");
    repo.git(&["add", "staged"]);
    repo.write("staged", "latest\n");
    repo.write("untracked", "another\n");
    repo.write("empty", "");
    let before = snapshot(&repo.0);
    let text = String::from_utf8(repo.spek(&[]).stdout).unwrap();
    assert_eq!(
        text,
        "AM staged    | 1 +\n?? empty     | 0\n?? untracked | 1 +\n 3 files changed, 2 insertions, 0 deletions\n"
    );
    assert_eq!(snapshot(&repo.0), before);
}

#[test]
fn handles_unusual_names_and_subdirectory_invocation() {
    let repo = Repo::new();
    repo.write("baseline", "same\n");
    repo.commit();
    assert!(repo.spek(&[]).stdout.is_empty());
    for name in [
        "with spaces",
        "line\nbreak",
        "tab\tquote\"",
        "-option",
        ":(glob)*",
    ] {
        repo.write(name, "new content\n");
    }
    fs::create_dir(repo.0.join("nested")).unwrap();
    repo.write("nested/inside", "inside\n");
    repo.git(&["config", "diff.relative", "true"]);
    let expected = repo.spek(&[]).stdout;
    let output = repo
        .command(env!("CARGO_BIN_EXE_spek"))
        .current_dir(repo.0.join("nested"))
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, expected);
    let text = String::from_utf8(expected).unwrap();
    assert_eq!(
        text.lines().filter(|line| line.starts_with("?? ")).count(),
        6,
        "{text}"
    );
    assert!(text.contains(r#""line\nbreak""#));
    assert!(text.ends_with("6 files changed, 6 insertions, 0 deletions\n"));
}

#[cfg(unix)]
#[test]
fn handles_non_utf8_names_symlinks_and_binary_files() {
    use std::ffi::OsString;
    use std::os::unix::{ffi::OsStringExt, fs::symlink};
    let repo = Repo::new();
    fs::write(
        repo.0.join(OsString::from_vec(b"non-utf8-\xff".to_vec())),
        b"content\n",
    )
    .unwrap();
    fs::write(repo.0.join("binary"), b"\0\x01\x02").unwrap();
    symlink("missing-target", repo.0.join("link")).unwrap();
    let text = String::from_utf8(repo.spek(&[]).stdout).unwrap();
    assert!(text.contains("non-utf8-\\377"), "{text}");
    assert!(
        text.lines()
            .any(|line| line.starts_with("?? binary") && line.ends_with("Bin 0 -> 3 bytes")),
        "{text}"
    );
    assert!(
        text.lines()
            .any(|line| line.starts_with("?? link") && line.ends_with("1 +")),
        "{text}"
    );
    assert!(text.ends_with("3 files changed, 2 insertions, 0 deletions\n"));
    assert!(!text.contains("missing-target"));
}

#[test]
fn reports_invalid_invocation_and_non_repository() {
    let repo = Repo::new();
    for args in [vec!["unknown"], vec!["--unknown"], vec!["diff", "extra"]] {
        let invalid = repo
            .command(env!("CARGO_BIN_EXE_spek"))
            .args(args)
            .output()
            .unwrap();
        assert!(!invalid.status.success());
        assert!(invalid.stdout.is_empty());
        assert!(String::from_utf8_lossy(&invalid.stderr).contains("usage:"));
    }
    fs::remove_dir_all(repo.0.join(".git")).unwrap();
    let output = repo.command(env!("CARGO_BIN_EXE_spek")).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not a git repository"));
}

#[test]
fn help_and_version_work_outside_a_repository() {
    let repo = Repo::new();
    fs::remove_dir_all(repo.0.join(".git")).unwrap();
    let help = repo.spek(&["--help"]).stdout;
    assert_eq!(help, repo.spek(&["-h"]).stdout);
    let help = String::from_utf8(help).unwrap();
    for expected in ["Usage: spek", "diff", "log", "-h, --help", "-V, --version"] {
        assert!(help.contains(expected), "{help}");
    }
    for flag in ["--version", "-V"] {
        assert_eq!(
            repo.spek(&[flag]).stdout,
            format!("spek {}\n", env!("CARGO_PKG_VERSION")).as_bytes()
        );
    }
}

#[test]
fn shows_renames_staged_deletions_and_tracked_binary_changes() {
    let repo = Repo::new();
    repo.write("old name", "one\ntwo\nthree\nfour\n");
    repo.write("gone", "removed\n");
    fs::write(repo.0.join("binary"), b"\0before").unwrap();
    repo.commit();
    repo.git(&["mv", "old name", "new name"]);
    repo.write("new name", "one\ntwo\nthree\nfour\nfive\n");
    repo.git(&["rm", "gone"]);
    fs::write(repo.0.join("binary"), b"\0after").unwrap();
    let before = snapshot(&repo.0);
    let text = String::from_utf8(repo.spek(&[]).stdout).unwrap();
    assert_eq!(
        text,
        " M binary               | Bin 7 -> 6 bytes\nD  gone                 | 1 -\nRM old name -> new name | 1 +\n 3 files changed, 1 insertion, 1 deletion\n"
    );
    assert_eq!(snapshot(&repo.0), before);
}

#[test]
fn scales_large_bars_and_treats_pathspec_characters_literally() {
    let repo = Repo::new();
    repo.write(":(glob)*", "old\n");
    repo.write("unrelated", "same\n");
    repo.commit();
    repo.write(":(glob)*", &"new\n".repeat(1000));
    repo.write("unrelated", "changed\n");
    let text = String::from_utf8(repo.spek(&[]).stdout).unwrap();
    assert!(
        text.ends_with("2 files changed, 1001 insertions, 2 deletions\n"),
        "{text}"
    );
    for row in text.lines().filter(|line| line.contains(" | ")) {
        let bar = row.split_whitespace().last().unwrap();
        assert!(bar.len() <= 50, "{row}");
        assert!(bar.contains('+') && bar.contains('-'), "{row}");
    }
}

#[test]
fn renamed_file_and_reused_source_are_counted_once() {
    let repo = Repo::new();
    repo.write("old", "one\ntwo\nthree\nfour\n");
    repo.commit();
    repo.git(&["mv", "old", "new"]);
    repo.write("old", "replacement\n");
    repo.write("new", "one\ntwo\nthree\nfour\nfive\n");
    let text = String::from_utf8(repo.spek(&[]).stdout).unwrap();
    assert!(
        text.ends_with("2 files changed, 2 insertions, 0 deletions\n"),
        "{text}"
    );
    assert!(
        text.lines()
            .any(|line| line.starts_with("RM old -> new") && line.ends_with("1 +")),
        "{text}"
    );
}

#[test]
fn respects_git_color_settings() {
    let repo = Repo::new();
    repo.write("file", "old\n");
    repo.commit();
    repo.write("file", "new\n");
    repo.git(&["config", "color.ui", "always"]);
    let colored = String::from_utf8(repo.spek(&[]).stdout).unwrap();
    assert!(
        colored.contains("\x1b[32m+\x1b[m\x1b[31m-\x1b[m"),
        "{colored:?}"
    );
    assert!(colored.starts_with(" \x1b[31mM\x1b[m file"));
    assert!(
        colored
            .ends_with("\n 1 file changed, \x1b[32m1 insertion\x1b[m, \x1b[31m1 deletion\x1b[m\n")
    );
    repo.git(&["config", "color.diff.new", "blue"]);
    assert!(
        repo.spek(&[])
            .stdout
            .windows(6)
            .any(|bytes| bytes == b"\x1b[34m+")
    );
    for setting in ["never", "auto"] {
        repo.git(&["config", "color.diff", setting]);
        repo.git(&["config", "color.status", setting]);
        assert!(!repo.spek(&[]).stdout.contains(&0x1b));
    }
}

#[test]
fn colors_each_status_column_using_git_status_settings() {
    let repo = Repo::new();
    repo.write("mixed", "original\n");
    repo.commit();
    repo.write("mixed", "staged\n");
    repo.git(&["add", "mixed"]);
    repo.write("mixed", "unstaged\n");
    repo.write("untracked", "new\n");
    repo.git(&["config", "color.status", "always"]);
    repo.git(&["config", "color.diff", "never"]);
    let output = String::from_utf8(repo.spek(&[]).stdout).unwrap();
    assert!(
        output.starts_with("\x1b[32mM\x1b[m\x1b[31mM\x1b[m mixed"),
        "{output:?}"
    );
    assert!(
        output.contains("\n\x1b[31m?\x1b[m\x1b[31m?\x1b[m untracked"),
        "{output:?}"
    );
    assert!(output.contains("2 +-\n"));
    repo.git(&["config", "color.status.added", "blue"]);
    repo.git(&["config", "color.status.changed", "yellow"]);
    repo.git(&["config", "color.status.untracked", "magenta"]);
    let output = String::from_utf8(repo.spek(&[]).stdout).unwrap();
    assert!(
        output.starts_with("\x1b[34mM\x1b[m\x1b[33mM\x1b[m mixed"),
        "{output:?}"
    );
    assert!(
        output.contains("\n\x1b[35m?\x1b[m\x1b[35m?\x1b[m untracked"),
        "{output:?}"
    );
}

#[test]
fn preserves_binary_sizes_and_omits_stats_for_pure_renames() {
    let repo = Repo::new();
    fs::write(repo.0.join("binary"), [0; 64]).unwrap();
    fs::write(repo.0.join("deleted"), [0; 16]).unwrap();
    repo.write("old", "unchanged\n");
    repo.commit();
    fs::write(repo.0.join("binary"), [0; 96]).unwrap();
    fs::remove_file(repo.0.join("deleted")).unwrap();
    fs::write(repo.0.join("new binary"), [0; 32]).unwrap();
    repo.git(&["mv", "old", "new"]);
    let before = snapshot(&repo.0);
    let text = String::from_utf8(repo.spek(&[]).stdout).unwrap();
    assert_eq!(
        text,
        " M binary     | Bin 64 -> 96 bytes\n D deleted    | Bin 16 -> 0 bytes\nR  old -> new\n?? new binary | Bin 0 -> 32 bytes\n 4 files changed, 0 insertions, 0 deletions\n"
    );
    assert_eq!(snapshot(&repo.0), before);
    repo.git(&["config", "color.diff", "always"]);
    let colored = String::from_utf8(repo.spek(&[]).stdout).unwrap();
    assert!(colored.contains("Bin \x1b[31m64\x1b[m -> \x1b[32m96\x1b[m bytes"));
    assert!(colored.contains("Bin \x1b[31m16\x1b[m -> \x1b[32m0\x1b[m bytes"));
    assert!(colored.contains("Bin \x1b[31m0\x1b[m -> \x1b[32m32\x1b[m bytes"));
    repo.git(&["config", "color.diff.old", "magenta"]);
    repo.git(&["config", "color.diff.new", "blue"]);
    let colored = String::from_utf8(repo.spek(&[]).stdout).unwrap();
    assert!(colored.contains("Bin \x1b[35m64\x1b[m -> \x1b[34m96\x1b[m bytes"));
}
