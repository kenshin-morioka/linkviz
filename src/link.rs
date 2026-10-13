use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::parse::{Rule, is_glob};

/// A single symlink after glob expansion, with its state on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub line: usize,
    /// Relative to the home directory.
    pub dest: PathBuf,
    /// Relative to the root directory.
    pub source: PathBuf,
    pub status: Status,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Linked,
    NotLinked,
    SourceMissing,
    WrongTarget(PathBuf),
    NotSymlink,
    Unreadable(String),
}

impl Status {
    pub fn is_ok(&self) -> bool {
        *self == Status::Linked
    }
}

pub fn resolve(rules: &[Rule], root: &Path, home: &Path) -> Vec<Link> {
    rules
        .iter()
        .flat_map(|rule| match expand(rule, root) {
            Ok(pairs) => pairs
                .into_iter()
                .map(|(dest, source)| Link {
                    line: rule.line,
                    status: status(&home.join(&dest), &root.join(&source)),
                    dest,
                    source,
                })
                .collect(),
            // Keep the unexpanded pattern so the failure stays visible in the output.
            Err(e) => vec![Link {
                line: rule.line,
                dest: rule.dest.clone().into(),
                source: rule.source.clone().into(),
                status: source_error(e),
            }],
        })
        .collect()
}

/// Expands `dir/*` into one pair per entry, skipping dotfiles like a default shell glob.
fn expand(rule: &Rule, root: &Path) -> io::Result<Vec<(PathBuf, PathBuf)>> {
    if !is_glob(&rule.source) {
        return Ok(vec![(rule.dest.clone().into(), rule.source.clone().into())]);
    }
    let dest_dir = glob_parent(&rule.dest);
    let source_dir = glob_parent(&rule.source);
    let mut names = fs::read_dir(root.join(source_dir))?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<io::Result<Vec<_>>>()?;
    names.retain(|name| !name.to_string_lossy().starts_with('.'));
    names.sort();
    Ok(names
        .into_iter()
        .map(|name| {
            (
                Path::new(dest_dir).join(&name),
                Path::new(source_dir).join(&name),
            )
        })
        .collect())
}

fn glob_parent(pattern: &str) -> &str {
    pattern
        .strip_suffix('*')
        .unwrap_or(pattern)
        .trim_end_matches('/')
}

fn status(dest: &Path, source: &Path) -> Status {
    if let Err(e) = fs::metadata(source) {
        return source_error(e);
    }
    let metadata = match fs::symlink_metadata(dest) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Status::NotLinked,
        Err(e) => return Status::Unreadable(e.to_string()),
    };
    if !metadata.file_type().is_symlink() {
        return Status::NotSymlink;
    }
    let target = match fs::read_link(dest) {
        Ok(target) => target,
        Err(e) => return Status::Unreadable(e.to_string()),
    };
    let target = match dest.parent() {
        Some(parent) if target.is_relative() => parent.join(target),
        _ => target,
    };
    if target == source || same_file(&target, source) {
        Status::Linked
    } else {
        Status::WrongTarget(target)
    }
}

fn source_error(e: io::Error) -> Status {
    if e.kind() == io::ErrorKind::NotFound {
        Status::SourceMissing
    } else {
        Status::Unreadable(e.to_string())
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    matches!((a.canonicalize(), b.canonicalize()), (Ok(a), Ok(b)) if a == b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    struct Fixture {
        _tmp: TempDir,
        root: PathBuf,
        home: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let tmp = TempDir::new().unwrap();
            let root = tmp.path().join("dotfiles");
            let home = tmp.path().join("home");
            fs::create_dir_all(&root).unwrap();
            fs::create_dir_all(&home).unwrap();
            Self {
                _tmp: tmp,
                root,
                home,
            }
        }

        fn file(&self, path: &str) {
            let path = self.root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "").unwrap();
        }

        fn link(&self, dest: &str, target: &Path) {
            let dest = self.home.join(dest);
            fs::create_dir_all(dest.parent().unwrap()).unwrap();
            symlink(target, dest).unwrap();
        }

        fn resolve(&self, definition: &str) -> Vec<Link> {
            resolve(&parse(definition).unwrap(), &self.root, &self.home)
        }
    }

    fn statuses(links: &[Link]) -> Vec<(String, Status)> {
        links
            .iter()
            .map(|l| (l.dest.display().to_string(), l.status.clone()))
            .collect()
    }

    #[test]
    fn detects_each_status() {
        let f = Fixture::new();
        for name in ["linked", "missing", "wrong", "real", "relative"] {
            f.file(name);
        }
        f.link("linked", &f.root.join("linked"));
        f.link("wrong", &f.root.join("linked"));
        fs::write(f.home.join("real"), "").unwrap();
        f.link("relative", Path::new("../dotfiles/relative"));

        let links = f.resolve(
            "linked:linked\nmissing:missing\nwrong:wrong\nreal:real\nrelative:relative\nabsent:absent",
        );

        assert_eq!(
            statuses(&links),
            vec![
                ("linked".into(), Status::Linked),
                ("missing".into(), Status::NotLinked),
                ("wrong".into(), Status::WrongTarget(f.root.join("linked"))),
                ("real".into(), Status::NotSymlink),
                ("relative".into(), Status::Linked),
                ("absent".into(), Status::SourceMissing),
            ]
        );
    }

    #[test]
    fn expands_glob_without_dotfiles() {
        let f = Fixture::new();
        f.file("skills/b/SKILL.md");
        f.file("skills/a/SKILL.md");
        f.file("skills/.hidden");
        f.link(".claude/skills/a", &f.root.join("skills/a"));

        let links = f.resolve(".claude/skills/*:skills/*");

        assert_eq!(
            statuses(&links),
            vec![
                (".claude/skills/a".into(), Status::Linked),
                (".claude/skills/b".into(), Status::NotLinked),
            ]
        );
        assert_eq!(links[1].source, PathBuf::from("skills/b"));
    }

    #[test]
    fn expands_glob_at_home_root() {
        let f = Fixture::new();
        f.file("home/.zshrc");
        f.file("home/bin");

        let links = f.resolve("*:home/*");

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].dest, PathBuf::from("bin"));
    }

    #[test]
    fn keeps_glob_with_missing_directory() {
        let f = Fixture::new();

        let links = f.resolve(".claude/skills/*:nowhere/*");

        assert_eq!(
            statuses(&links),
            vec![(".claude/skills/*".into(), Status::SourceMissing)]
        );
    }

    #[test]
    fn reports_unreadable_source_as_error() {
        use std::os::unix::fs::PermissionsExt;

        let f = Fixture::new();
        f.file("locked/file");
        let locked = f.root.join("locked");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        // Permission bits do not apply to root, so the scenario cannot be reproduced there.
        let reproducible = fs::read_dir(&locked).is_err();

        let links = f.resolve("file:locked/file\nskills/*:locked/*");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();

        if !reproducible {
            return;
        }
        assert!(
            links
                .iter()
                .all(|l| matches!(l.status, Status::Unreadable(_))),
            "{links:?}"
        );
    }
}
