mod common;

use common::{Repo, snapshot};
use std::fs;

fn commit(repo: &Repo, subject: &str, date: &str) {
    let output = repo
        .command("git")
        .args(["commit", "--allow-empty", "-qm", subject])
        .env("GIT_AUTHOR_NAME", "Example User")
        .env("GIT_AUTHOR_EMAIL", "Original.Author@example.com")
        .env("GIT_AUTHOR_DATE", date)
        .env("GIT_COMMITTER_DATE", date)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}

#[test]
fn renders_exact_author_dates_subjects_and_decorations_without_changes() {
    let repo = Repo::new();
    repo.git(&["symbolic-ref", "HEAD", "refs/heads/main"]);
    commit(
        &repo,
        "fix: preserve PR number (#291)",
        "2026-09-15T22:34:00+0200",
    );
    let first = String::from_utf8(repo.git(&["rev-parse", "--short", "HEAD"])).unwrap();
    commit(
        &repo,
        "feat: support tabs | and separators · intact",
        "2026-09-16T19:41:00+0200",
    );
    let second = String::from_utf8(repo.git(&["rev-parse", "--short", "HEAD"])).unwrap();
    repo.git(&["tag", "v0.1"]);
    repo.git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    repo.write(
        ".mailmap",
        "Rewritten <rewritten@example.com> Example User <Original.Author@example.com>\n",
    );
    repo.git(&["config", "log.mailmap", "true"]);
    repo.git(&["config", "log.showSignature", "true"]);
    repo.git(&["notes", "add", "-m", "This note must not appear"]);
    fs::create_dir(repo.0.join("nested")).unwrap();
    let before = snapshot(&repo.0);
    let text = String::from_utf8(repo.spek(&["log"]).stdout).unwrap();
    let expected = format!(
        "spek — 2 commits\n\nSep 16, 2026\n • feat: support tabs | and separators · intact  {}\n   19:41 · Example User <Original.Author@example.com> · HEAD -> main, tag: v0.1, origin/main\n\nSep 15, 2026\n • fix: preserve PR number (#291)  {}\n   22:34 · Example User <Original.Author@example.com>\n",
        second.trim(),
        first.trim(),
    );
    assert_eq!(text, expected);
    let nested = repo
        .command(env!("CARGO_BIN_EXE_spek"))
        .arg("log")
        .current_dir(repo.0.join("nested"))
        .output()
        .unwrap();
    assert!(nested.status.success(), "{nested:?}");
    assert_eq!(nested.stdout, text.as_bytes());
    assert_eq!(snapshot(&repo.0), before);
}

#[test]
fn handles_empty_repositories_and_errors() {
    let repo = Repo::new();
    assert_eq!(repo.spek(&["log"]).stdout, "spek — 0 commits\n".as_bytes());
    let invalid = repo
        .command(env!("CARGO_BIN_EXE_spek"))
        .args(["log", "extra"])
        .output()
        .unwrap();
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("usage:"));
    fs::remove_dir_all(repo.0.join(".git")).unwrap();
    let invalid = repo
        .command(env!("CARGO_BIN_EXE_spek"))
        .arg("log")
        .output()
        .unwrap();
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("not a git repository"));
}

#[test]
fn counts_reachable_history_including_merges_and_handles_detached_head() {
    let repo = Repo::new();
    repo.git(&["symbolic-ref", "HEAD", "refs/heads/main"]);
    repo.write("file", "initial\n");
    repo.commit();
    repo.git(&["switch", "-qc", "topic"]);
    commit(&repo, "topic change (#291)", "2026-09-15T10:00:00+0000");
    repo.git(&["switch", "-q", "main"]);
    commit(&repo, "main change", "2026-09-16T10:00:00+0000");
    repo.git(&["merge", "--no-ff", "topic", "-m", "Merge pull request #291"]);
    let text = String::from_utf8(repo.spek(&["log"]).stdout).unwrap();
    assert!(text.starts_with("spek — 4 commits\n"), "{text}");
    assert!(text.contains("Merge pull request #291"));
    assert!(text.contains("topic change (#291)"));
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("   ") && line.contains(" · "))
            .count(),
        4
    );
    repo.git(&["switch", "--detach", "HEAD~2"]);
    let text = String::from_utf8(repo.spek(&["log"]).stdout).unwrap();
    assert!(text.starts_with("spek — 1 commit\n"), "{text}");
    assert!(text.contains(" · HEAD\n"), "{text}");
    assert!(!text.contains("main change"));
}

#[test]
fn log_colors_respect_git_configuration_and_plain_pipes() {
    let repo = Repo::new();
    repo.git(&["symbolic-ref", "HEAD", "refs/heads/main"]);
    repo.write("file", "initial\n");
    repo.commit();
    repo.git(&["tag", "v0.1"]);
    repo.git(&["branch", "origin/local"]);
    repo.git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    let plain = repo.spek(&["log"]).stdout;
    assert!(!plain.contains(&0x1b));
    repo.git(&["config", "color.ui", "always"]);
    let colored = String::from_utf8(repo.spek(&["log"]).stdout).unwrap();
    assert!(colored.contains("\x1b[36m•\x1b[m"));
    assert!(colored.contains("\x1b[33mSep "));
    assert!(!colored.contains("\x1b[2m"));
    assert!(colored.contains("\x1b[36m"));
    assert!(!colored.contains("\x1b[1m"));
    assert!(colored.contains("\x1b[37m"));
    assert!(colored.contains("Spek Test <test@example.com>"));
    assert!(!colored.contains("\x1b[1mSpek Test"));
    assert!(colored.contains("\x1b[36mHEAD\x1b[m -> \x1b[32mmain\x1b[m"));
    assert!(colored.contains("tag: \x1b[33mv0.1\x1b[m"));
    assert!(colored.contains("\x1b[31morigin/main\x1b[m"));
    assert!(colored.contains("\x1b[32morigin/local\x1b[m"));
    let stripped = colored
        .replace("\x1b[97m", "")
        .replace("\x1b[33m", "")
        .replace("\x1b[36m", "")
        .replace("\x1b[1m", "")
        .replace("\x1b[37m", "")
        .replace("\x1b[32m", "")
        .replace("\x1b[31m", "")
        .replace("\x1b[m", "");
    assert_eq!(stripped.as_bytes(), plain);
    for value in ["never", "auto"] {
        repo.git(&["config", "color.ui", value]);
        assert_eq!(repo.spek(&["log"]).stdout, plain);
    }
}

#[test]
fn redirected_log_does_not_start_a_pager() {
    let repo = Repo::new();
    repo.write("file", "initial\n");
    repo.commit();
    let expected = repo.spek(&["log"]).stdout;
    repo.git(&["config", "core.pager", "exit 42"]);
    let output = repo
        .command(env!("CARGO_BIN_EXE_spek"))
        .arg("log")
        .env("GIT_PAGER", "exit 43")
        .env("PAGER", "exit 44")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert_eq!(output.stdout, expected);
}

#[test]
fn keeps_hash_after_long_subject() {
    let repo = Repo::new();
    commit(
        &repo,
        "this is a deliberately long subject that cannot fit beside its hash",
        "2026-09-17T10:00:00+0000",
    );
    let output = repo
        .command(env!("CARGO_BIN_EXE_spek"))
        .arg("log")
        .env("COLUMNS", "24")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("this is a deliberately long subject that cannot fit beside its hash"));
    let hash = String::from_utf8(repo.git(&["rev-parse", "--short", "HEAD"]))
        .unwrap()
        .trim()
        .to_owned();
    assert!(text.contains(&hash));
}

#[test]
fn groups_consecutive_calendar_dates_and_highlights_only_pr_numbers() {
    let repo = Repo::new();
    commit(&repo, "fix: old change", "2025-09-06T20:57:00+0200");
    commit(
        &repo,
        "fix: same day (#291) and #42",
        "2026-09-06T20:57:00+0200",
    );
    commit(&repo, "Merge pull request #291", "2026-09-06T22:34:00+0200");
    commit(&repo, "feat: latest", "2026-09-07T19:41:00+0200");
    let plain = String::from_utf8(repo.spek(&["log"]).stdout).unwrap();
    assert_eq!(plain.matches("Sep 6, 2026").count(), 1);
    assert_eq!(plain.matches("Sep 7, 2026").count(), 1);
    assert_eq!(plain.matches("Sep 6, 2025").count(), 1);
    assert!(plain.contains("   22:34 · Example User <Original.Author@example.com>"));
    assert!(plain.contains("   20:57 · Example User <Original.Author@example.com>"));
    assert!(!plain.contains("\n\n\n"));
    repo.git(&["config", "color.ui", "always"]);
    let colored = String::from_utf8(repo.spek(&["log"]).stdout).unwrap();
    assert!(colored.contains("\x1b[97mMerge pull request \x1b[36m#291\x1b[m\x1b[97m"));
    assert!(colored.contains("(\x1b[36m#291\x1b[m\x1b[97m) and \x1b[36m#42\x1b[m\x1b[97m"));
}
