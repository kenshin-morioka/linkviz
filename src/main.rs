mod html;
mod link;
mod parse;
mod render;

use std::env;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{self, Path, PathBuf};
use std::process::{Command, ExitCode};

use clap::{Parser, ValueEnum};

use render::Style;

/// Visualize a symlink definition file as a tree or network diagram, with each link's state on disk.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Definition file of `<dest>:<source>` lines
    #[arg(default_value = "symlinks.txt")]
    file: PathBuf,

    /// Base directory of source paths [default: directory of FILE]
    #[arg(long)]
    root: Option<PathBuf>,

    /// Base directory of dest paths [default: $HOME]
    #[arg(long)]
    home: Option<PathBuf>,

    /// Color the tree output; `auto` respects NO_COLOR and whether stdout is a terminal
    #[arg(long, value_enum, default_value_t = ColorChoice::Auto)]
    color: ColorChoice,

    /// Open an interactive network diagram in the browser instead of printing the tree
    #[arg(long)]
    open: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum ColorChoice {
    Auto,
    Always,
    Never,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("linkviz: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<String, String> {
    let definition =
        fs::read_to_string(&cli.file).map_err(|e| format!("{}: {e}", cli.file.display()))?;
    let rules = parse::parse(&definition).map_err(|e| format!("{}: {e}", cli.file.display()))?;

    let root = match cli.root {
        Some(root) => root,
        None => cli
            .file
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .to_path_buf(),
    };
    let home_given = cli.home.is_some();
    let home = cli
        .home
        .or_else(|| env::var_os("HOME").map(PathBuf::from))
        .ok_or("HOME is not set; pass --home")?;
    // Links are created with absolute source paths, so compare against absolute paths too.
    let root = path::absolute(&root).map_err(|e| format!("{}: {e}", root.display()))?;
    let home = path::absolute(&home).map_err(|e| format!("{}: {e}", home.display()))?;

    let links = link::resolve(&rules, &root, &home);
    let home_label = if home_given {
        home.display().to_string()
    } else {
        "~".to_string()
    };

    if cli.open {
        // The diagram shows full paths in its header, so node labels stay short.
        let html = html::render(
            &links,
            &html::Sides {
                home_label: &if home_given {
                    dir_name(&home)
                } else {
                    home_label.clone()
                },
                home: &home,
                root_label: &dir_name(&root),
                root: &root,
            },
        );
        let path = open_in_browser(&html)?;
        return Ok(format!("Opened {}\n", path.display()));
    }

    let style = Style {
        color: use_color(cli.color),
    };
    Ok(render::render(&links, &home_label, &style))
}

fn dir_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// The temp dir may be shared between users (e.g. `/tmp` on Linux), so each run creates a fresh,
/// owner-only file with a random name instead of reusing a predictable path that others could
/// pre-create or symlink. The file is kept for the browser and left to the OS temp cleanup.
fn open_in_browser(html: &str) -> Result<PathBuf, String> {
    let mut file = tempfile::Builder::new()
        .prefix("linkviz-")
        .suffix(".html")
        .tempfile()
        .map_err(|e| format!("failed to create a temp file: {e}"))?;
    file.write_all(html.as_bytes())
        .map_err(|e| format!("{}: {e}", file.path().display()))?;
    let (_, path) = file.keep().map_err(|e| e.to_string())?;
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let status = Command::new(opener)
        .arg(&path)
        .status()
        .map_err(|e| format!("failed to run `{opener}`: {e}"))?;
    if !status.success() {
        return Err(format!(
            "`{opener}` exited with {status}; open {} manually",
            path.display()
        ));
    }
    Ok(path)
}

fn use_color(choice: ColorChoice) -> bool {
    match choice {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => {
            env::var_os("NO_COLOR").is_none_or(|v| v.is_empty()) && io::stdout().is_terminal()
        }
    }
}
