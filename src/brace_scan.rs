#[derive(Clone, Copy, Debug, Default)]
pub struct BraceScanState {
    pub depth: i32,
    pub in_block_comment: bool,
    pub string_delimiter: Option<char>,
}

impl BraceScanState {
    pub fn scan_line(&mut self, line: &str, hash_starts_comment: bool) {
        let chars: Vec<char> = line.chars().collect();
        let mut index = 0;

        while index < chars.len() {
            if self.consume_block_comment_end(&chars, &mut index) {
                continue;
            }
            if self.consume_string_escape(&chars, &mut index) {
                continue;
            }
            if self.consume_string_end(chars[index]) {
                index += 1;
                continue;
            }
            if self.consume_block_comment_start(&chars, &mut index) {
                continue;
            }
            if self.ready_for_code_tokens()
                && starts_line_comment(&chars, index, hash_starts_comment)
            {
                break;
            }

            self.consume_code_char(chars[index]);
            index += 1;
        }
    }

    fn consume_block_comment_end(&mut self, chars: &[char], index: &mut usize) -> bool {
        if !self.in_block_comment || !matches_pair(chars, *index, '*', '/') {
            return false;
        }

        self.in_block_comment = false;
        *index += 2;
        true
    }

    fn consume_string_escape(&self, chars: &[char], index: &mut usize) -> bool {
        if self.string_delimiter.is_none() || chars[*index] != '\\' {
            return false;
        }

        *index += 2;
        true
    }

    fn consume_string_end(&mut self, ch: char) -> bool {
        match self.string_delimiter {
            Some(delimiter) if ch == delimiter => {
                self.string_delimiter = None;
                true
            }
            _ => false,
        }
    }

    fn consume_block_comment_start(&mut self, chars: &[char], index: &mut usize) -> bool {
        if !self.ready_for_code_tokens() || !matches_pair(chars, *index, '/', '*') {
            return false;
        }

        self.in_block_comment = true;
        *index += 2;
        true
    }

    fn consume_code_char(&mut self, ch: char) {
        if !self.ready_for_code_tokens() {
            return;
        }

        match ch {
            '"' | '\'' => self.string_delimiter = Some(ch),
            '{' => self.depth += 1,
            '}' => self.depth -= 1,
            _ => {}
        }
    }

    fn ready_for_code_tokens(&self) -> bool {
        !self.in_block_comment && self.string_delimiter.is_none()
    }
}

fn starts_line_comment(chars: &[char], index: usize, hash_starts_comment: bool) -> bool {
    matches_pair(chars, index, '/', '/') || (hash_starts_comment && chars[index] == '#')
}

fn matches_pair(chars: &[char], index: usize, left: char, right: char) -> bool {
    index + 1 < chars.len() && chars[index] == left && chars[index + 1] == right
}
