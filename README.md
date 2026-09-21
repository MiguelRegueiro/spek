# Spek

A small, read-only Rust CLI for compact Git status, diff statistics, and commit history. Requires `git` on `PATH`; no Rust dependencies.

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

`spek log` prints the history reachable from `HEAD`, including merges, in Git's default order:

```text
spek — 2 commits

Sep 16, 2026
 • feat(preview): add configurable tab width (#291)  7ca52c9
   19:41 · Example User <user@example.com> · HEAD -> main, origin/main

Sep 15, 2026
 • fix(sixel): correct image rendering in tmux  dfbfc15
   22:34 · Example User <user@example.com>
```

The header counts all commits in that history. A calendar-date header appears whenever the author date changes in Git’s history order. Each commit keeps a two-line layout: the bold subject first, then the time, stored author name and email, short hash (without mailmap rewriting), and Git's branch/tag/remote decorations. Subjects remain bold/yellow, with only `#` and its digits highlighted cyan in PR references. Subjects retain PR numbers as written; no GitHub metadata is fetched. An empty repository shows `spek — 0 commits`.

Log output uses `color.ui`: the commit subject and short hash are bold/yellow, author and email at normal brightness, and date in normal-brightness grey; HEAD is cyan, local branches green, remote branches red, and tags yellow. In a terminal, it uses Git's configured pager (`GIT_PAGER`, then `core.pager`, then `PAGER`, with Git's default, usually `less`, as fallback). When `LESS` is unset, Spek supplies Git-like `LESS=FRX`, preserving ANSI colors and leaving the visible page on screen after `q`. An explicitly set `LESS` value is preserved. Setting the pager to `cat` or an empty value disables it. Piped or redirected output never starts a pager. `spek --help` lists commands, and `spek --version` prints the package version.

The implementation is dependency-free: `src/main.rs` dispatches commands, `src/cli.rs` handles arguments, and `src/diff.rs` / `src/log.rs` implement their respective views using the Git CLI.

```sh
cargo fmt --check
cargo check
cargo test
```
