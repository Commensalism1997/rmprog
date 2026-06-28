# rmprog

![Screenshot](example.png)

## Syntax

### NOTE: If `PATH` is a symlink to a directory, only the symlink will be deleted.

```bash
rmprog [-v] <PATH>...
```

It will work on both files and directories, with progress bar obviously being only visible on the latter. `-v` will print out every deleted file much like `rm -v`.

## Installation

Provided you have cargo and have .cargo/bin in $PATH:

```bash
cargo install --locked --git https://github.com/Commensalism1997/rmprog.git
```

Async ver. (EXPERIMENTAL):
```bash
cargo install --locked --git https://github.com/Commensalism1997/rmprog.git --branch async
```
