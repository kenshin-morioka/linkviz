use std::collections::BTreeMap;
use std::fmt::Write;

use crate::link::{Kind, Link, Status};

pub struct Style {
    pub color: bool,
}

impl Style {
    fn paint(&self, code: &str, text: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }
}

#[derive(Default)]
struct Node<'a> {
    children: BTreeMap<String, Node<'a>>,
    links: Vec<&'a Link>,
}

impl<'a> Node<'a> {
    fn insert(&mut self, link: &'a Link) {
        let node = link.dest.components().fold(self, |node, component| {
            let name = component.as_os_str().to_string_lossy().into_owned();
            node.children.entry(name).or_default()
        });
        node.links.push(link);
    }
}

struct Row<'a> {
    label: String,
    links: &'a [&'a Link],
}

pub fn render(links: &[Link], root_label: &str, style: &Style) -> String {
    let mut tree = Node::default();
    for link in links {
        tree.insert(link);
    }
    let mut rows = Vec::new();
    collect_rows(&tree, "", &mut rows);
    let width = rows
        .iter()
        .filter(|row| !row.links.is_empty())
        .map(|row| row.label.chars().count())
        .max()
        .unwrap_or(0);

    let mut out = format!("{}\n", style.paint("1", root_label));
    for row in &rows {
        out.push_str(&row_line(row, width, style));
        out.push('\n');
    }
    out.push('\n');
    out.push_str(&summary(links, style));
    out.push('\n');
    out
}

fn collect_rows<'a>(node: &'a Node<'a>, prefix: &str, rows: &mut Vec<Row<'a>>) {
    let count = node.children.len();
    for (i, (name, child)) in node.children.iter().enumerate() {
        let last = i + 1 == count;
        let connector = if last { "└── " } else { "├── " };
        let slash = if child.links.is_empty() { "/" } else { "" };
        rows.push(Row {
            label: format!("{prefix}{connector}{name}{slash}"),
            links: &child.links,
        });
        let indent = if last { "    " } else { "│   " };
        collect_rows(child, &format!("{prefix}{indent}"), rows);
    }
}

fn row_line(row: &Row, width: usize, style: &Style) -> String {
    let Some((first, rest)) = row.links.split_first() else {
        return style.paint("34", &row.label);
    };
    let padding = " ".repeat(width - row.label.chars().count());
    let mut line = format!(
        "{}{padding}  → {}  {}",
        row.label,
        style.paint("2", &first.source.display().to_string()),
        status_label(&first.status, style),
    );
    // Several rules resolving to the same dest means one silently overwrites the other.
    for other in rest {
        let _ = write!(
            line,
            "  {}",
            style.paint(
                "31",
                &format!(
                    "⚠ also defined on line {} → {}",
                    other.line,
                    other.source.display()
                )
            )
        );
    }
    line
}

fn status_label(status: &Status, style: &Style) -> String {
    style.paint(kind_color(status.kind()), &status.label())
}

fn kind_color(kind: Kind) -> &'static str {
    match kind {
        Kind::Ok => "32",
        Kind::Pending => "33",
        Kind::Problem => "31",
    }
}

fn summary(links: &[Link], style: &Style) -> String {
    let count = |kind: Kind| links.iter().filter(|l| l.status.kind() == kind).count();
    format!(
        "{} links: {}, {}, {}",
        links.len(),
        style.paint("32", &format!("{} linked", count(Kind::Ok))),
        style.paint("33", &format!("{} not linked", count(Kind::Pending))),
        style.paint("31", &format!("{} problems", count(Kind::Problem))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn link(line: usize, dest: &str, source: &str, status: Status) -> Link {
        Link {
            line,
            dest: PathBuf::from(dest),
            source: PathBuf::from(source),
            status,
        }
    }

    #[test]
    fn renders_aligned_tree() {
        let links = vec![
            link(1, ".zshrc", "zsh/.zshrc", Status::Linked),
            link(2, ".config/nvim", "nvim", Status::NotLinked),
            link(
                3,
                ".config/starship.toml",
                "starship.toml",
                Status::SourceMissing,
            ),
            link(4, ".claude/skills/a", "skills/a", Status::Linked),
            link(5, ".claude/skills/a", "private/skills/a", Status::Linked),
        ];

        let out = render(&links, "~", &Style { color: false });

        assert_eq!(
            out,
            "\
~
├── .claude/
│   └── skills/
│       └── a          → skills/a  ✓  ⚠ also defined on line 5 → private/skills/a
├── .config/
│   ├── nvim           → nvim  not linked
│   └── starship.toml  → starship.toml  source missing
└── .zshrc             → zsh/.zshrc  ✓

5 links: 3 linked, 1 not linked, 1 problems
"
        );
    }
}
