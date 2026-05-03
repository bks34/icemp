use std::io::BufRead;

pub struct Lrc {
    lrc_file_path: String,
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
                Lrc {
                    lrc_file_path: path,
                    data,
                }
            }
            Err(e) => {
                #[cfg(debug_assertions)]
                println!("Lrc::from_path error: {}", e);
                Lrc {
                    lrc_file_path: path,
                    data: Vec::new(),
                }
            }
        }
    }

    pub fn get_lyrics(&self, time: u64) -> String {
        let mut ans: &str = "";
        for (t, w) in self.data.iter() {
            if time > *t {
                ans = w.as_ref();
            } else {
                break;
            }
        }
        ans.into()
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
