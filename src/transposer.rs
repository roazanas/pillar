pub fn transpose(strings: Vec<&str>, add_empty_lines: bool) -> Vec<String> {
    // Convert to chars vectors to avoid encoding problems
    let char_matrix: Vec<Vec<char>> = strings.iter().map(|s| s.chars().collect()).collect();

    let max_len = char_matrix.iter().map(|row| row.len()).max().unwrap_or(0);
    let mut result = Vec::with_capacity(max_len);

    for i in 0..max_len {
        let mut new_row = String::with_capacity(strings.len() * 2);

        for (row_idx, row) in char_matrix.iter().enumerate() {
            if i < row.len() {
                new_row.push(row[i]);
            } else {
                new_row.push(' ');
            }

            if !add_empty_lines {
                continue;
            }

            if row_idx < char_matrix.len() - 1 {
                new_row.push(' ');
            }
        }
        result.push(new_row);
    }

    result
}
