#[derive(Clone, Default)]
pub(super) struct AnsiSgrState {
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
    blink: bool,
    reverse: bool,
    hidden: bool,
    strike: bool,
    fg: Option<Vec<u16>>,
    bg: Option<Vec<u16>>,
}

impl AnsiSgrState {
    pub(super) fn apply_escape_sequence(&mut self, sequence: &str) {
        if !sequence.starts_with('[') || !sequence.ends_with('m') {
            return;
        }

        let body = &sequence[1..sequence.len() - 1];
        let params: Vec<u16> = if body.is_empty() {
            vec![0]
        } else {
            body.split(';')
                .map(|part| {
                    if part.is_empty() {
                        0
                    } else {
                        part.parse().unwrap_or(0)
                    }
                })
                .collect()
        };

        let mut i = 0;
        while i < params.len() {
            match params[i] {
                0 => *self = Self::default(),
                1 => self.bold = true,
                2 => self.dim = true,
                3 => self.italic = true,
                4 => self.underline = true,
                5 | 6 => self.blink = true,
                7 => self.reverse = true,
                8 => self.hidden = true,
                9 => self.strike = true,
                22 => {
                    self.bold = false;
                    self.dim = false;
                }
                23 => self.italic = false,
                24 => self.underline = false,
                25 => self.blink = false,
                27 => self.reverse = false,
                28 => self.hidden = false,
                29 => self.strike = false,
                30..=37 | 90..=97 => self.fg = Some(vec![params[i]]),
                39 => self.fg = None,
                40..=47 | 100..=107 => self.bg = Some(vec![params[i]]),
                49 => self.bg = None,
                38 | 48 => {
                    let color_target = params[i];
                    let Some(mode) = params.get(i + 1).copied() else {
                        i += 1;
                        continue;
                    };

                    match mode {
                        5 => {
                            let Some(value) = params.get(i + 2).copied() else {
                                i += 1;
                                continue;
                            };
                            if color_target == 38 {
                                self.fg = Some(vec![38, 5, value]);
                            } else {
                                self.bg = Some(vec![48, 5, value]);
                            }
                            i += 2;
                        }
                        2 => {
                            let (Some(r), Some(g), Some(b)) = (
                                params.get(i + 2).copied(),
                                params.get(i + 3).copied(),
                                params.get(i + 4).copied(),
                            ) else {
                                i += 1;
                                continue;
                            };
                            if color_target == 38 {
                                self.fg = Some(vec![38, 2, r, g, b]);
                            } else {
                                self.bg = Some(vec![48, 2, r, g, b]);
                            }
                            i += 4;
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }

    pub(super) fn restore_sequence(&self) -> Option<String> {
        let mut params: Vec<String> = Vec::new();

        if self.bold {
            params.push("1".into());
        }
        if self.dim {
            params.push("2".into());
        }
        if self.italic {
            params.push("3".into());
        }
        if self.underline {
            params.push("4".into());
        }
        if self.blink {
            params.push("5".into());
        }
        if self.reverse {
            params.push("7".into());
        }
        if self.hidden {
            params.push("8".into());
        }
        if self.strike {
            params.push("9".into());
        }
        if let Some(fg) = &self.fg {
            params.extend(fg.iter().map(u16::to_string));
        }
        if let Some(bg) = &self.bg {
            params.extend(bg.iter().map(u16::to_string));
        }

        if params.is_empty() {
            None
        } else {
            Some(format!("\x1b[{}m", params.join(";")))
        }
    }
}
