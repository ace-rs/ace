use super::Template;

/// Candidate match: (open_pos, name_start, name_end).
type Match = (usize, usize, usize);

#[derive(Clone, Copy)]
enum Phase {
    Text,
    MaybeOpen,
    Name,
    MaybeClose,
}

pub struct Parser {
    phase: Phase,
    open_pos: usize,
    name_start: usize,
    lit_start: usize,
}

impl Parser {
    pub fn new() -> Self {
        Self {
            phase: Phase::Text,
            open_pos: 0,
            name_start: 0,
            lit_start: 0,
        }
    }

    /// Drive the full parse loop, pushing segments into `tpl`.
    pub fn parse_all<'a>(&mut self, input: &'a str, tpl: &mut Template<'a>) {
        for (i, byte) in input.bytes().enumerate() {
            let Some((open, ns, ne)) = self.feed(i, byte) else {
                continue;
            };

            let name = input[ns..ne].trim();
            if !is_valid_name(name) {
                continue;
            }

            tpl.push_literal(&input[self.lit_start..open]);
            tpl.push_placeholder(name);
            self.lit_start = ne + 2; // skip past `}}`
        }

        tpl.push_literal(&input[self.lit_start..]);
    }

    fn feed(&mut self, i: usize, byte: u8) -> Option<Match> {
        match self.phase {
            Phase::Text => self.on_text(i, byte),
            Phase::MaybeOpen => self.on_maybe_open(i, byte),
            Phase::Name => self.on_name(byte),
            Phase::MaybeClose => self.on_maybe_close(i, byte),
        }
    }

    fn on_text(&mut self, i: usize, byte: u8) -> Option<Match> {
        if byte == b'{' {
            self.phase = Phase::MaybeOpen;
            self.open_pos = i;
        }
        None
    }

    fn on_maybe_open(&mut self, i: usize, byte: u8) -> Option<Match> {
        self.phase = if byte == b'{' {
            self.name_start = i + 1;
            Phase::Name
        } else {
            Phase::Text
        };
        None
    }

    fn on_name(&mut self, byte: u8) -> Option<Match> {
        if byte == b'\n' {
            self.phase = Phase::Text;
        } else if byte == b'}' {
            self.phase = Phase::MaybeClose;
        }
        None
    }

    fn on_maybe_close(&mut self, i: usize, byte: u8) -> Option<Match> {
        self.phase = Phase::Text;
        (byte == b'}').then(|| (self.open_pos, self.name_start, i - 1))
    }
}

fn is_valid_name(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
