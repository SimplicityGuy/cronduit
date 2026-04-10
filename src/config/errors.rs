use std::path::PathBuf;

#[derive(Debug)]
pub struct ConfigError {
    pub file: PathBuf,
    pub line: usize, // 1-indexed
    pub col: usize,  // 1-indexed
    pub message: String,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.line == 0 {
            write!(f, "{}: error: {}", self.file.display(), self.message)
        } else {
            write!(
                f,
                "{}:{}:{}: error: {}",
                self.file.display(),
                self.line,
                self.col,
                self.message
            )
        }
    }
}

/// Convert a byte offset in `source` into 1-indexed (line, col).
pub fn byte_offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut col = 1usize;
    for (i, c) in source.char_indices() {
        if i >= offset {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_col_basic() {
        let s = "abc\ndef\nghi";
        assert_eq!(byte_offset_to_line_col(s, 0), (1, 1));
        assert_eq!(byte_offset_to_line_col(s, 4), (2, 1));
        assert_eq!(byte_offset_to_line_col(s, 5), (2, 2));
        assert_eq!(byte_offset_to_line_col(s, 8), (3, 1));
    }

    #[test]
    fn display_with_line_col() {
        let e = ConfigError {
            file: "x.toml".into(),
            line: 3,
            col: 7,
            message: "boom".into(),
        };
        assert_eq!(format!("{e}"), "x.toml:3:7: error: boom");
    }
}
