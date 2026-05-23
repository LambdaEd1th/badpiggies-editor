pub struct SerializedFile {
    pub data: Vec<u8>,
}

impl SerializedFile {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Heuristic to find TextAsset-like data (Name string + bytes).
    /// This is a simplified scanner since full SerializedFile parsing is very complex.
    pub fn extract_text_assets(&self) -> Vec<(String, Vec<u8>)> {
        let mut results = Vec::new();
        let mut i = 0;
        
        // Search for potential TextAsset objects
        // In Bad Piggies bundles, these are usually at the end of the file or easily identifiable.
        // We look for: [Name Length (u32)] [Name Bytes] [Padding to 4 bytes] [Data Length (u32)] [Data Bytes]
        
        while i + 8 < self.data.len() {
            let name_len = u32::from_le_bytes(self.data[i..i+4].try_into().unwrap()) as usize;
            if name_len > 0 && name_len < 128 && i + 4 + name_len + 4 < self.data.len() {
                if let Ok(name) = String::from_utf8(self.data[i+4..i+4+name_len].to_vec()) {
                    // Check if name looks like a level name
                    if name.contains("Level") || name.contains("Sandbox") || name.contains("Episode") {
                        let padding = (4 - (name_len % 4)) % 4;
                        let data_len_pos = i + 4 + name_len + padding;
                        
                        if data_len_pos + 4 <= self.data.len() {
                            let data_len = u32::from_le_bytes(self.data[data_len_pos..data_len_pos+4].try_into().unwrap()) as usize;
                            if data_len > 0 && data_len < 10 * 1024 * 1024 && data_len_pos + 4 + data_len <= self.data.len() {
                                let content = self.data[data_len_pos+4..data_len_pos+4+data_len].to_vec();
                                // Check for level magic: first byte should be a count of objects, usually > 0
                                if !content.is_empty() {
                                    results.push((format!("{}.bytes", name), content));
                                    i = data_len_pos + 4 + data_len;
                                    continue;
                                }
                            }
                        }
                    }
                }
            }
            i += 1;
        }
        
        results
    }
}
