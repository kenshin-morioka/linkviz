use std::fmt::Write;
use std::path::Path;

use crate::link::Link;

const TEMPLATE: &str = include_str!("diagram.html");
const PLACEHOLDER: &str = "/*LINKVIZ_DATA*/null";

/// Labels and full paths of the two sides of the diagram.
pub struct Sides<'a> {
    pub home_label: &'a str,
    pub home: &'a Path,
    pub root_label: &'a str,
    pub root: &'a Path,
}

pub fn render(links: &[Link], sides: &Sides) -> String {
    let mut data = format!(
        "{{\"home\":{},\"homePath\":{},\"root\":{},\"rootPath\":{},\"links\":[",
        json_string(sides.home_label),
        json_string(&sides.home.display().to_string()),
        json_string(sides.root_label),
        json_string(&sides.root.display().to_string()),
    );
    for (i, link) in links.iter().enumerate() {
        if i > 0 {
            data.push(',');
        }
        let _ = write!(
            data,
            "{{\"dest\":{},\"source\":{},\"line\":{},\"kind\":\"{}\",\"status\":{}}}",
            json_string(&link.dest.display().to_string()),
            json_string(&link.source.display().to_string()),
            link.line,
            link.status.kind().as_str(),
            json_string(&link.status.label()),
        );
    }
    data.push_str("]}");
    TEMPLATE.replacen(PLACEHOLDER, &data, 1)
}

/// Encodes a JSON string literal that is also safe to embed inside a `<script>` element.
fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '<' | '>' | '&' | '\u{2028}' | '\u{2029}' => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::link::Status;
    use std::path::PathBuf;

    #[test]
    fn template_has_single_placeholder() {
        assert_eq!(TEMPLATE.matches(PLACEHOLDER).count(), 1);
    }

    #[test]
    fn embeds_links_as_json() {
        let links = vec![Link {
            line: 3,
            dest: PathBuf::from(".zshrc"),
            source: PathBuf::from("zsh/.zshrc"),
            status: Status::NotLinked,
        }];
        let sides = Sides {
            home_label: "~",
            home: Path::new("/Users/me"),
            root_label: "dotfiles",
            root: Path::new("/Users/me/dotfiles"),
        };

        let html = render(&links, &sides);

        assert!(!html.contains(PLACEHOLDER));
        assert!(html.contains(
            r#"{"home":"~","homePath":"/Users/me","root":"dotfiles","rootPath":"/Users/me/dotfiles","links":[{"dest":".zshrc","source":"zsh/.zshrc","line":3,"kind":"pending","status":"not linked"}]}"#
        ));
    }

    #[test]
    fn escapes_script_breaking_characters() {
        let encoded = json_string("</script>\"\\\n");

        assert!(!encoded.contains(['<', '>', '\n']), "{encoded}");
        assert!(encoded.ends_with(r#"\"\\\u000a""#), "{encoded}");
    }
}
