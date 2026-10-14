//! Splits the Stats API TCP stream (concatenated JSON objects, no delimiter) into objects.

/// Drop the buffer if a single object grows past this (corrupt or hostile stream). Real messages
/// are a few KB; at ~21 KB/s a stray `{` costs under a minute of data before resynchronising.
pub const MAX_BUFFERED: usize = 1024 * 1024;

#[derive(Debug, Default)]
pub struct JsonFramer {
    buf: Vec<u8>,
    scanned: usize,
    start: usize,
    depth: u32,
    in_string: bool,
    escaped: bool,
    resets: u32,
}

impl JsonFramer {
    /// How many times buffered data was dropped since the last call (then resets the count).
    pub fn take_resets(&mut self) -> u32 {
        std::mem::take(&mut self.resets)
    }

    /// Feed raw bytes; returns every JSON object completed by this chunk.
    pub fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(chunk);
        let mut out = Vec::new();
        for i in self.scanned..self.buf.len() {
            let c = self.buf[i];
            if self.depth > 0 && i - self.start > MAX_BUFFERED {
                // Corrupt or never-closing object: drop it and resynchronise on the next `{`.
                self.depth = 0;
                self.in_string = false;
                self.escaped = false;
                self.resets += 1;
            }
            if self.in_string {
                if self.escaped {
                    self.escaped = false;
                } else if c == b'\\' {
                    self.escaped = true;
                } else if c == b'"' {
                    self.in_string = false;
                }
                continue;
            }
            match c {
                b'"' if self.depth > 0 => self.in_string = true,
                b'{' => {
                    if self.depth == 0 {
                        self.start = i;
                    }
                    self.depth += 1;
                }
                b'}' if self.depth > 0 => {
                    self.depth -= 1;
                    if self.depth == 0 {
                        out.push(String::from_utf8_lossy(&self.buf[self.start..=i]).into_owned());
                    }
                }
                _ => {}
            }
        }
        self.compact();
        out
    }

    fn compact(&mut self) {
        if self.depth == 0 {
            self.buf.clear();
        } else {
            self.buf.drain(..self.start);
        }
        self.start = 0;
        self.scanned = self.buf.len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_all(framer: &mut JsonFramer, chunks: &[&[u8]]) -> Vec<String> {
        chunks.iter().flat_map(|c| framer.push(c)).collect()
    }

    #[test]
    fn splits_objects_across_byte_boundaries() {
        let stream = br#"{"Event":"A","Data":{"x":1}}{"Event":"B","Data":"{\"y\":\"}{\"}"}"#;
        let mut framer = JsonFramer::default();
        let chunks: Vec<&[u8]> = stream.chunks(1).collect();
        let out = push_all(&mut framer, &chunks);
        assert_eq!(
            out,
            vec![
                r#"{"Event":"A","Data":{"x":1}}"#.to_string(),
                r#"{"Event":"B","Data":"{\"y\":\"}{\"}"}"#.to_string(),
            ]
        );
    }

    #[test]
    fn ignores_whitespace_between_objects() {
        let mut framer = JsonFramer::default();
        let out = push_all(&mut framer, &[b"{\"a\":1}\r\n  {\"b\":", b"2}\n"]);
        assert_eq!(out, vec!["{\"a\":1}".to_string(), "{\"b\":2}".to_string()]);
    }

    #[test]
    fn keeps_multibyte_characters_split_across_chunks() {
        let bytes = "{\"Name\":\"Zé\"}".as_bytes();
        let split = bytes.len() - 3; // inside the two-byte 'é'
        let mut framer = JsonFramer::default();
        let out = push_all(&mut framer, &[&bytes[..split], &bytes[split..]]);
        assert_eq!(out, vec!["{\"Name\":\"Zé\"}".to_string()]);
    }

    #[test]
    fn a_stray_brace_costs_under_two_megabytes_and_is_reported() {
        // ~21 KB/s at PacketSendRate=10: 16 MB would swallow a whole match unnoticed.
        let mut framer = JsonFramer::default();
        framer.push(b"{\"truncated\":");
        let valid = b"{\"Event\":\"UpdateState\",\"Data\":{}}".repeat(2 * 1024 * 1024 / 34);
        let recovered = framer.push(&valid);
        assert!(!recovered.is_empty(), "framer resynchronised");
        assert_eq!(framer.take_resets(), 1);
        assert_eq!(framer.take_resets(), 0, "taking clears the count");
    }

    #[test]
    fn resets_when_an_object_never_closes() {
        let mut framer = JsonFramer::default();
        framer.push(b"{\"open\":");
        let filler = vec![b'1'; MAX_BUFFERED];
        assert!(framer.push(&filler).is_empty());
        let out = framer.push(b"{\"ok\":true}");
        assert_eq!(out, vec!["{\"ok\":true}".to_string()]);
    }
}
