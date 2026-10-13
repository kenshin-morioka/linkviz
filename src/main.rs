mod link;
mod parse;
mod render;

use std::env;
use std::fs;
use std::io::{self, IsTerminal};
use std::path::{self, Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, ValueEnum};

use render::Style;

/// Visualize a symlink definition file as a tree, with each link's state on disk.
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

    #[arg(long, value_enum, default_value_t = ColorChoice::Auto)]
    color: ColorChoice,
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
    let root_label = if home_given {
        home.display().to_string()
    } else {
        "~".to_string()
    };
    let style = Style {
        color: use_color(cli.color),
    };
    Ok(render::render(&links, &root_label, &style))
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
