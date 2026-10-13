# linkviz

Visualize a dotfiles symlink definition file as a tree, with each link's state on disk.

```text
~
├── .claude/
│   ├── CLAUDE.md          → agents/AGENTS.md  ✓
│   └── skills/
│       ├── review         → agents/skills/review  ✓
│       └── tdd            → agents/skills/tdd  not linked
├── .config/
│   ├── nvim               → nvim  ✓
│   └── starship.toml      → starship.toml  points to /somewhere/else
└── .zshrc                 → zsh/.zshrc  exists, not a symlink

6 links: 3 linked, 1 not linked, 2 problems
```

## Install

```sh
cargo install --path .
```

## Usage

```sh
linkviz [FILE] [--root DIR] [--home DIR] [--color auto|always|never]
```

- `FILE`: definition file (default: `symlinks.txt`)
- `--root`: base directory of source paths (default: directory of `FILE`)
- `--home`: base directory of dest paths (default: `$HOME`)
- `--color`: `auto` respects `NO_COLOR` and whether stdout is a terminal

## Definition format

```text
# <dest relative to home>:<source relative to root>
.zshrc:zsh/.zshrc
.config/nvim:nvim

# A trailing `*` on both sides links each entry of the source directory (dotfiles excluded)
.claude/skills/*:agents/skills/*
```

- Blank lines and lines starting with `#` are ignored
- Each line is split on the first `:`

## Statuses

| Status | Meaning |
| --- | --- |
| `✓` | dest is a symlink to source |
| `not linked` | dest does not exist |
| `source missing` | source does not exist |
| `points to <path>` | dest is a symlink to somewhere else |
| `exists, not a symlink` | dest is a regular file or directory |
| `⚠ also defined on line N` | another line resolves to the same dest |

## License

MIT
