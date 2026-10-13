use std::fmt;

/// One `<dest>:<source>` line of a symlink definition file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub line: usize,
    pub dest: String,
    pub source: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseError {}

pub fn parse(input: &str) -> Result<Vec<Rule>, ParseError> {
    input
        .lines()
        .enumerate()
        .filter_map(|(i, raw)| {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            Some(parse_line(i + 1, line))
        })
        .collect()
}

fn parse_line(line_no: usize, line: &str) -> Result<Rule, ParseError> {
    let error = |message: &str| ParseError {
        line: line_no,
        message: message.to_string(),
    };
    // Split on the first `:` only, matching `IFS=: read -r dst src` in shell.
    let (dest, source) = line
        .split_once(':')
        .ok_or_else(|| error("expected `<dest>:<source>`"))?;
    if dest.is_empty() || source.is_empty() {
        return Err(error("dest and source must not be empty"));
    }
    if is_glob(dest) != is_glob(source) {
        return Err(error("a trailing `*` must be on both dest and source"));
    }
    Ok(Rule {
        line: line_no,
        dest: dest.to_string(),
        source: source.to_string(),
    })
}

/// A path whose last component is `*`, meaning "each entry of the parent directory".
pub fn is_glob(path: &str) -> bool {
    path == "*" || path.ends_with("/*")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_comments_and_blank_lines() {
        let rules = parse("# comment\n\n  # indented\n.zshrc:zsh/.zshrc\n").unwrap();
        assert_eq!(
            rules,
            vec![Rule {
                line: 4,
                dest: ".zshrc".into(),
                source: "zsh/.zshrc".into(),
            }]
        );
    }

    #[test]
    fn splits_on_first_colon() {
        let rules = parse("a:b:c").unwrap();
        assert_eq!(rules[0].dest, "a");
        assert_eq!(rules[0].source, "b:c");
    }

    #[test]
    fn rejects_line_without_colon() {
        let err = parse("ok:ok\nbroken").unwrap_err();
        assert_eq!(err.line, 2);
    }

    #[test]
    fn rejects_empty_side() {
        assert!(parse(":src").is_err());
        assert!(parse("dest:").is_err());
    }

    #[test]
    fn rejects_glob_on_one_side() {
        assert!(parse(".claude/skills/*:agents/skills").is_err());
        assert!(parse(".claude/skills:agents/skills/*").is_err());
        assert!(parse(".claude/skills/*:agents/skills/*").is_ok());
    }

    #[test]
    fn detects_glob() {
        assert!(is_glob("*"));
        assert!(is_glob("a/b/*"));
        assert!(!is_glob("a/b*"));
        assert!(!is_glob("a/*/b"));
    }
}
