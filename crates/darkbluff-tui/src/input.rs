use unicode_width::UnicodeWidthStr;

#[derive(Debug, Default, Clone)]
pub struct CommandInput {
    value: String,
    cursor: usize,
}

impl CommandInput {
    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn display_cursor(&self) -> u16 {
        UnicodeWidthStr::width(&self.value[..self.byte_idx()]) as u16
    }

    /// 光标是否在行尾（斜杠补全仅在此情形下弹出）。
    pub fn cursor_at_end(&self) -> bool {
        self.cursor == self.value.chars().count()
    }

    /// 整体替换内容并把光标移到行尾（补全时使用）。
    pub fn set_value(&mut self, v: String) {
        self.value = v;
        self.cursor = self.value.chars().count();
    }

    pub fn insert(&mut self, c: char) {
        let idx = self.byte_idx();
        self.value.insert(idx, c);
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        // 定位光标前一字符的字节起点，原地移除（复用缓冲，无需重建整串）。
        if let Some((byte_idx, _)) = self.value.char_indices().nth(self.cursor - 1) {
            self.value.remove(byte_idx);
            self.cursor -= 1;
        }
    }

    pub fn delete(&mut self) {
        if self.cursor >= self.value.chars().count() {
            return;
        }
        let byte_idx = self.byte_idx();
        self.value.remove(byte_idx);
    }

    pub fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.value.chars().count());
    }

    pub fn jump_start(&mut self) {
        self.cursor = 0;
    }

    pub fn jump_end(&mut self) {
        self.cursor = self.value.chars().count();
    }

    pub fn submit(&mut self) -> String {
        self.cursor = 0;
        std::mem::take(&mut self.value)
    }

    fn byte_idx(&self) -> usize {
        self.value
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.cursor)
            .unwrap_or(self.value.len())
    }
}
