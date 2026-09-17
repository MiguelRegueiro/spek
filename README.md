# Spek

A small, read-only Rust CLI combining Git's short status with diff statistics. Requires `git` on `PATH`; no Rust dependencies.

```sh
cargo install --path .
spek
spek diff
```

Both commands show the same compact list for the whole repository, even from a subdirectory:

```text
 M Cargo.lock      | 30 ------------------------------
 M Cargo.toml      |  8 +++-----
?? src/new_file.rs | 42 ++++++++++++++++++++++++++++++++++++++++++
 3 files changed, 45 insertions, 35 deletions
```

- The two status columns come from Git: index changes first, working-tree changes second. Untracked files use `??`, and renames show `old -> new`.
- Counts represent the net change from `HEAD` to the working tree, or from the empty tree before the first commit. Files with staged and unstaged edits that cancel out still appear with a zero count.
- Untracked text files count as insertions, respecting Git's ignore rules. Empty files count as changed files with zero lines.
- Binary files show Git’s size information, such as `Bin 64 -> 96 bytes`, and contribute to the file count, not line totals. Embedded untracked repositories show `directory` without line counts.
- Pure renames with no line changes omit the stat portion.
- Bars scale to at most 50 characters. Unusual filenames are quoted and escaped to keep each entry on one line.
- Clean repositories produce no output. No patch hunks or file contents are printed.

Git supplies status and line counts using NUL-separated output. Spek leaves the index and working tree untouched, disables external diff drivers and text conversion, and respects Git's color settings without starting a pager. Status letters use Git’s staged, unstaged, untracked, and unmerged colors (`color.status.*`). In terminals, insertion bars and summary counts are green, and deletion bars and summary counts are red by default; piped output stays plain unless color is explicitly forced in Git configuration.

```sh
cargo fmt --check
cargo test
```
