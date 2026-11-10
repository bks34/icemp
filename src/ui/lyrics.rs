use std::io::BufRead;

pub struct Lrc {
    data: Vec<(u64, String)>,
}

impl Lrc {
    pub fn from_path(path: String) -> Self {
        match std::fs::File::open(path.clone()) {
            Ok(file) => {
                let mut data: Vec<(u64, String)> = Vec::new();
                let reader = std::io::BufReader::new(file);
                for line_res in reader.lines() {
                    let _line = line_res.unwrap();
                    let line = _line.trim();
                    match Self::parse_time(line) {
                        Some((time, idx)) => {
                            data.push((time, line[idx..].trim().to_string()));
                        }
                        None => continue,
                    }
                }
                Lrc { data }
            }
            Err(_e) => {
                #[cfg(debug_assertions)]
                println!("Lrc::from_path error: {}", _e);
                Lrc { data: Vec::new() }
            }
        }
    }

    pub fn lines(&self) -> &[(u64, String)] {
        &self.data
    }

    pub fn current_index(&self, time: u64) -> Option<usize> {
        if self.data.is_empty() {
            return None;
        }

        let next = self
            .data
            .partition_point(|(timestamp, _)| *timestamp <= time);
        Some(next.saturating_sub(1))
    }

    pub fn visible_range(&self, current: usize, radius: usize) -> std::ops::Range<usize> {
        if self.data.is_empty() {
            return 0..0;
        }

        let start = current.saturating_sub(radius);
        let end = (current + radius + 1).min(self.data.len());
        start..end
    }

    pub fn get_lyrics(&self, time: u64) -> String {
        self.current_index(time)
            .and_then(|index| self.data.get(index))
            .map(|(_, text)| text.clone())
            .unwrap_or_default()
    }

    fn parse_time(line: &str) -> Option<(u64, usize)> {
        let chars = line.char_indices();
        let mut time: u64 = 0;
        let mut text_start_idx: usize = 0;
        for (idx, c) in chars {
            if c == ']' {
                text_start_idx = idx + 1;
                break;
            }
            if idx == 1 {
                if c < '0' || c > '9' {
                    return None;
                }
                time += (c as u64 - '0' as u64) * 10 * 60 * 1000;
            }
            if idx == 2 {
                time += (c as u64 - '0' as u64) * 60 * 1000;
            }
            if idx == 4 {
                time += (c as u64 - '0' as u64) * 10 * 1000;
            }
            if idx == 5 {
                time += (c as u64 - '0' as u64) * 1000;
            }
            if idx == 7 {
                time += (c as u64 - '0' as u64) * 100;
            }
            if idx == 8 {
                time += (c as u64 - '0' as u64) * 10;
            }
        }
        Some((time, text_start_idx))
    }
}

#[cfg(test)]
mod tests {
    use super::Lrc;

    #[test]
    fn parses_lrclib_style_timestamp() {
        assert_eq!(Lrc::parse_time("[00:12.34] lyric"), Some((12_340, 10)));
        assert_eq!(Lrc::parse_time("[01:02.05] lyric"), Some((62_050, 10)));
        assert_eq!(Lrc::parse_time("not a timestamp"), None);
    }

    #[test]
    fn finds_current_lyric_and_visible_window() {
        let lrc = Lrc {
            data: vec![
                (1_000, "one".into()),
                (2_000, "two".into()),
                (3_000, "three".into()),
                (4_000, "four".into()),
            ],
        };

        assert_eq!(lrc.current_index(0), Some(0));
        assert_eq!(lrc.current_index(2_500), Some(1));
        assert_eq!(lrc.current_index(9_000), Some(3));
        assert_eq!(lrc.visible_range(2, 1), 1..4);
        assert_eq!(lrc.visible_range(0, 2), 0..3);
    }
}
