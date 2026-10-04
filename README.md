# Spek

A small, read-only Rust CLI for compact Git status, diff statistics, and commit history. Requires `git` on `PATH`; no Rust dependencies.

```sh
cargo install --path .

spek
spek log
spek exclude draft.md notes.md
spek exclude --list
spek include draft.md
spek include all
```

`spek` and `spek diff` show repository-wide changes, even when run from a subdirectory. They never modify the index, working tree, Git configuration, or ignore files. Clean repositories produce no output.

`spek log` shows the history reachable from `HEAD`, including merges, with dates, authors, refs, and Git's configured pager.

## Exclusions

`spek exclude <path>...` hides exact repository-relative paths from Spek's diff view only. Use `include` to restore paths, or `include all` to clear the list.

Exclusions are private local state, stored under `$XDG_STATE_HOME/spek` (or `~/.local/state/spek`). They are kept separately for each checkout and do not change the repository.

```sh
cargo fmt --check
cargo check
cargo test
```
