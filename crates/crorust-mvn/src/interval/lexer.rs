#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IntervalToken {
    Open(bool),
    Comma,
    Close(bool),
    Version(String),
}

pub struct IntervalLexer<I> {
    buffer: String,
    last: Option<IntervalToken>,
    iter: I,
}

impl<I: Iterator<Item = char>> IntervalLexer<I> {
    pub fn new(iter: I) -> Self {
        Self {
            buffer: String::new(),
            last: None,
            iter,
        }
    }
}

impl<I: Iterator<Item = char>> Iterator for IntervalLexer<I> {
    type Item = IntervalToken;

    fn next(&mut self) -> Option<IntervalToken> {
        if let Some(last) = self.last.take() {
            return Some(last);
        }
        loop {
            let token = if let Some(c) = self.iter.next() {
                Some(match c {
                    '[' => IntervalToken::Open(true),
                    '(' => IntervalToken::Open(false),
                    ']' => IntervalToken::Close(true),
                    ')' => IntervalToken::Close(false),
                    ',' => IntervalToken::Comma,
                    _ => {
                        self.buffer.push(c);
                        continue;
                    }
                })
            } else {
                None
            };
            if self.buffer.is_empty() {
                return token;
            } else {
                let version = self.buffer.clone();
                self.buffer.clear();
                self.last = token;
                return Some(IntervalToken::Version(version));
            }
        }
    }
}
