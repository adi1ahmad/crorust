#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Item<'a> {

    Newline,

    Text(Style, &'a str),

    Hyperlink(Style, &'a str, &'a str),

    Indentation(usize),

    QuoteIndent,

    BulletPoint,

    NumberedPoint(&'a str),

    Separator,

    CodeBlock(&'a str, &'a str),
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Style {

    pub heading: bool,

    pub quoted: bool,

    pub code: bool,

    pub strong: bool,

    pub underline: bool,

    pub strikethrough: bool,

    pub italics: bool,

    pub small: bool,

    pub raised: bool,
}

pub struct Parser<'a> {

    s: &'a str,

    start_of_line: bool,

    style: Style,
}

impl<'a> Parser<'a> {
    pub fn new(s: &'a str) -> Self {
        Self {
            s,
            start_of_line: true,
            style: Style::default(),
        }
    }

    fn numbered_list(&mut self) -> Option<Item<'a>> {
        let n_digits = self.s.chars().take_while(|c| c.is_ascii_digit()).count();
        if n_digits > 0 && self.s.chars().skip(n_digits).take(2).eq(". ".chars()) {
            let number = &self.s[..n_digits];
            self.s = &self.s[(n_digits + 2)..];
            self.start_of_line = false;
            return Some(Item::NumberedPoint(number));
        }
        None
    }

    fn code_block(&mut self) -> Option<Item<'a>> {
        if let Some(language_start) = self.s.strip_prefix("```")
            && let Some(newline) = language_start.find('\n')
        {
            let language = &language_start[..newline];
            let code_start = &language_start[newline + 1..];
            return if let Some(end) = code_start.find("\n```") {
                let code = &code_start[..end].trim();
                self.s = &code_start[end + 4..];
                self.start_of_line = false;
                Some(Item::CodeBlock(language, code))
            } else {
                self.s = "";
                Some(Item::CodeBlock(language, code_start))
            };
        }
        None
    }

    fn inline_code(&mut self) -> Option<Item<'a>> {
        if let Some(rest) = self.s.strip_prefix('`') {
            self.s = rest;
            self.start_of_line = false;
            self.style.code = true;
            let rest_of_line = &self.s[..self.s.find('\n').unwrap_or(self.s.len())];
            return if let Some(end) = rest_of_line.find('`') {
                let item = Item::Text(self.style, &self.s[..end]);
                self.s = &self.s[end + 1..];
                self.style.code = false;
                Some(item)
            } else {
                let end = rest_of_line.len();
                let item = Item::Text(self.style, rest_of_line);
                self.s = &self.s[end..];
                self.style.code = false;
                Some(item)
            };
        }
        None
    }

    fn url(&mut self) -> Option<Item<'a>> {
        if self.s.starts_with('<') {
            let this_line = &self.s[..self.s.find('\n').unwrap_or(self.s.len())];
            if let Some(url_end) = this_line.find('>') {
                let url = &self.s[1..url_end];
                self.s = &self.s[url_end + 1..];
                self.start_of_line = false;
                return Some(Item::Hyperlink(self.style, url, url));
            }
        }

        if self.s.starts_with('[') {
            let this_line = &self.s[..self.s.find('\n').unwrap_or(self.s.len())];
            if let Some(bracket_end) = this_line.find(']') {
                let text = &this_line[1..bracket_end];
                if this_line[bracket_end + 1..].starts_with('(')
                    && let Some(parens_end) = this_line[bracket_end + 2..].find(')')
                {
                    let parens_end = bracket_end + 2 + parens_end;
                    let url = &self.s[bracket_end + 2..parens_end];
                    self.s = &self.s[parens_end + 1..];
                    self.start_of_line = false;
                    return Some(Item::Hyperlink(self.style, text, url));
                }
            }
        }
        None
    }
}

impl<'a> Iterator for Parser<'a> {
    type Item = Item<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.s.is_empty() {
                return None;
            }

            if self.s.starts_with('\n') {
                self.s = &self.s[1..];
                self.start_of_line = true;
                self.style = Style::default();
                return Some(Item::Newline);
            }

            if self.s.starts_with("\\\n") && self.s.len() >= 2 {
                self.s = &self.s[2..];
                self.start_of_line = false;
                continue;
            }

            if self.s.starts_with('\\') && self.s.len() >= 2 {
                let text = &self.s[1..2];
                self.s = &self.s[2..];
                self.start_of_line = false;
                return Some(Item::Text(self.style, text));
            }

            if self.start_of_line {

                if self.s.starts_with(' ') {
                    let length = self.s.find(|c| c != ' ').unwrap_or(self.s.len());
                    self.s = &self.s[length..];
                    self.start_of_line = true;
                    return Some(Item::Indentation(length));
                }

                if let Some(after) = self.s.strip_prefix("# ") {
                    self.s = after;
                    self.start_of_line = false;
                    self.style.heading = true;
                    continue;
                }

                if let Some(after) = self.s.strip_prefix("> ") {
                    self.s = after;
                    self.start_of_line = true;
                    self.style.quoted = true;
                    return Some(Item::QuoteIndent);
                }

                if self.s.starts_with("- ") {
                    self.s = &self.s[2..];
                    self.start_of_line = false;
                    return Some(Item::BulletPoint);
                }

                if let Some(item) = self.numbered_list() {
                    return Some(item);
                }

                if let Some(after) = self.s.strip_prefix("---") {
                    self.s = after.trim_start_matches('-');
                    self.s = self.s.strip_prefix('\n').unwrap_or(self.s);
                    self.start_of_line = false;
                    return Some(Item::Separator);
                }

                if let Some(item) = self.code_block() {
                    return Some(item);
                }
            }

            if let Some(item) = self.inline_code() {
                return Some(item);
            }

            if let Some(rest) = self.s.strip_prefix('*') {
                self.s = rest;
                self.start_of_line = false;
                self.style.strong = !self.style.strong;
                continue;
            }
            if let Some(rest) = self.s.strip_prefix('_') {
                self.s = rest;
                self.start_of_line = false;
                self.style.underline = !self.style.underline;
                continue;
            }
            if let Some(rest) = self.s.strip_prefix('~') {
                self.s = rest;
                self.start_of_line = false;
                self.style.strikethrough = !self.style.strikethrough;
                continue;
            }
            if let Some(rest) = self.s.strip_prefix('/') {
                self.s = rest;
                self.start_of_line = false;
                self.style.italics = !self.style.italics;
                continue;
            }
            if let Some(rest) = self.s.strip_prefix('$') {
                self.s = rest;
                self.start_of_line = false;
                self.style.small = !self.style.small;
                continue;
            }
            if let Some(rest) = self.s.strip_prefix('^') {
                self.s = rest;
                self.start_of_line = false;
                self.style.raised = !self.style.raised;
                continue;
            }

            if let Some(item) = self.url() {
                return Some(item);
            }

            let end = self
                .s
                .find(&['*', '`', '~', '_', '/', '$', '^', '\\', '<', '[', '\n'][..])
                .map_or_else(|| self.s.len(), |special| special.max(1));

            let item = Item::Text(self.style, &self.s[..end]);
            self.s = &self.s[end..];
            self.start_of_line = false;
            return Some(item);
        }
    }
}

#[test]
fn test_easy_mark_parser() {
    let items: Vec<_> = Parser::new("~strikethrough `code`~").collect();
    assert_eq!(
        items,
        vec![
            Item::Text(
                Style {
                    strikethrough: true,
                    ..Default::default()
                },
                "strikethrough "
            ),
            Item::Text(
                Style {
                    code: true,
                    strikethrough: true,
                    ..Default::default()
                },
                "code"
            ),
        ]
    );
}
