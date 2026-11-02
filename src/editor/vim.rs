use super::Buffer;

// Vim's word classes: blanks, keyword characters, and runs of other punctuation.
fn class(c: char) -> u8 {
    if c.is_whitespace() {
        0
    } else if c.is_alphanumeric() || c == '_' {
        1
    } else {
        2
    }
}

fn chars(b: &Buffer, row: usize) -> Vec<char> {
    b.lines
        .get(row)
        .map_or_else(Vec::new, |l| l.chars().collect())
}

pub fn first_non_blank(line: &str) -> usize {
    line.chars().take_while(|c| c.is_whitespace()).count()
}

pub fn word_next(b: &Buffer, mut row: usize, mut col: usize) -> (usize, usize) {
    let line = chars(b, row);
    if let Some(&c) = line.get(col) {
        let start = class(c);
        if start != 0 {
            while col < line.len() && class(line[col]) == start {
                col += 1;
            }
        }
    }
    loop {
        let line = chars(b, row);
        while col < line.len() && class(line[col]) == 0 {
            col += 1;
        }
        if col < line.len() {
            return (row, col);
        }
        if row + 1 >= b.lines.len() {
            return (row, line.len().saturating_sub(1));
        }
        row += 1;
        col = 0;
        // An empty line counts as a word of its own, as in Vim.
        if b.lines[row].is_empty() {
            return (row, 0);
        }
    }
}

pub fn word_prev(b: &Buffer, mut row: usize, mut col: usize) -> (usize, usize) {
    loop {
        if col == 0 {
            if row == 0 {
                return (0, 0);
            }
            row -= 1;
            col = b.line_len(row);
            if col == 0 {
                return (row, 0);
            }
        }
        col -= 1;
        if class(chars(b, row)[col]) != 0 {
            break;
        }
    }
    let line = chars(b, row);
    let start = class(line[col]);
    while col > 0 && class(line[col - 1]) == start {
        col -= 1;
    }
    (row, col)
}

pub fn word_end(b: &Buffer, mut row: usize, mut col: usize) -> (usize, usize) {
    col += 1;
    loop {
        let line = chars(b, row);
        while col < line.len() && class(line[col]) == 0 {
            col += 1;
        }
        if col < line.len() {
            let start = class(line[col]);
            while col + 1 < line.len() && class(line[col + 1]) == start {
                col += 1;
            }
            return (row, col);
        }
        if row + 1 >= b.lines.len() {
            return (row, line.len().saturating_sub(1));
        }
        row += 1;
        col = 0;
    }
}
