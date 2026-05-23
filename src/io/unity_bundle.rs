use std::io::{self, Read, Cursor};
use lz4_flex::block::decompress;

#[derive(Debug, Clone)]
pub struct BundleNode {
    pub offset: u64,
    pub size: u64,
    pub _flags: u32,
    pub path: String,
}

#[derive(Debug, Clone)]
struct BlockInfo {
    uncompressed_size: u32,
    compressed_size: u32,
    flags: u16,
}

pub struct UnityBundleReader {
    data: Vec<u8>,
    nodes: Vec<BundleNode>,
    blocks: Vec<BlockInfo>,
    data_offset: u64,
}

impl UnityBundleReader {
    pub fn new(data: Vec<u8>) -> io::Result<Self> {
        let mut r = Cursor::new(&data);
        
        // 1. Header
        let mut magic = [0u8; 8];
        r.read_exact(&mut magic)?;
        if &magic != b"UnityFS\0" {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid UnityFS magic"));
        }

        let version = read_u32_be(&mut r)?;
        if version != 6 {
             // BP uses version 6
        }

        let _engine_version = read_string(&mut r)?;
        let _build_version = read_string(&mut r)?;
        let _bundle_size = read_u64_be(&mut r)?;
        
        let compressed_blocks_info_size = read_u32_be(&mut r)?;
        let uncompressed_blocks_info_size = read_u32_be(&mut r)?;
        let flags = read_u32_be(&mut r)?;

        let header_size = r.position();

        // 2. Blocks Info
        let blocks_info_data = if flags & 0x80 != 0 {
            // Blocks info at the end
            return Err(io::Error::new(io::ErrorKind::Unsupported, "Blocks info at end not supported yet"));
        } else {
            let mut compressed = vec![0u8; compressed_blocks_info_size as usize];
            r.read_exact(&mut compressed)?;
            
            let compression_type = flags & 0x3F;
            decompress_data(&compressed, uncompressed_blocks_info_size as usize, compression_type)?
        };

        let data_offset = header_size + compressed_blocks_info_size as u64;

        let mut br = Cursor::new(blocks_info_data);
        let mut _hash = [0u8; 16];
        br.read_exact(&mut _hash)?;

        let block_count = read_u32_be(&mut br)?;
        let mut blocks = Vec::with_capacity(block_count as usize);
        for _ in 0..block_count {
            blocks.push(BlockInfo {
                uncompressed_size: read_u32_be(&mut br)?,
                compressed_size: read_u32_be(&mut br)?,
                flags: read_u16_be(&mut br)?,
            });
        }

        let node_count = read_u32_be(&mut br)?;
        let mut nodes = Vec::with_capacity(node_count as usize);
        for _ in 0..node_count {
            nodes.push(BundleNode {
                offset: read_u64_be(&mut br)?,
                size: read_u64_be(&mut br)?,
                _flags: read_u32_be(&mut br)?,
                path: read_string(&mut br)?,
            });
        }

        Ok(Self {
            data,
            nodes,
            blocks,
            data_offset,
        })
    }

    pub fn list_files(&self) -> &[BundleNode] {
        &self.nodes
    }

    pub fn read_file(&self, node_path: &str) -> io::Result<Vec<u8>> {
        let node = self.nodes.iter().find(|n| n.path == node_path)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("File not found in bundle: {}", node_path)))?;

        let mut result = Vec::with_capacity(node.size as usize);
        let mut current_offset = 0u64;
        let mut block_start_offset = self.data_offset;

        for block in &self.blocks {
            let block_end = current_offset + block.uncompressed_size as u64;
            
            // Check if this block contains any part of our node
            if block_end > node.offset && current_offset < node.offset + node.size {
                let compressed_data = &self.data[block_start_offset as usize..(block_start_offset + block.compressed_size as u64) as usize];
                
                let compression_type = (block.flags & 0x3F) as u32;
                let decompressed = decompress_data(compressed_data, block.uncompressed_size as usize, compression_type)?;

                // Extract relevant part of decompressed block
                let start_in_block = (node.offset.saturating_sub(current_offset)) as usize;
                let end_in_block = ((node.offset + node.size).min(block_end) - current_offset) as usize;
                
                result.extend_from_slice(&decompressed[start_in_block..end_in_block]);
            }

            block_start_offset += block.compressed_size as u64;
            current_offset = block_end;
            
            if current_offset >= node.offset + node.size {
                break;
            }
        }

        Ok(result)
    }
}

fn decompress_data(compressed: &[u8], uncompressed_size: usize, compression_type: u32) -> io::Result<Vec<u8>> {
    match compression_type {
        0 => Ok(compressed.to_vec()), // Raw
        1 => { // LZMA
            let mut result = Vec::with_capacity(uncompressed_size);
            
            // Unity LZMA blocks are [5-byte properties] [raw data]
            // Standard LZMA is [5-byte properties] [8-byte uncompressed size] [raw data]
            if compressed.len() < 5 {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "LZMA block too small"));
            }

            let mut lzma_header = [0u8; 13];
            lzma_header[0..5].copy_from_slice(&compressed[0..5]);
            lzma_header[5..13].copy_from_slice(&(uncompressed_size as u64).to_le_bytes());
            
            let mut full_data = Vec::with_capacity(13 + compressed.len() - 5);
            full_data.extend_from_slice(&lzma_header);
            full_data.extend_from_slice(&compressed[5..]);
            
            let mut r = Cursor::new(full_data);
            lzma_rs::lzma_decompress(&mut r, &mut result)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("LZMA decompression failed: {}", e)))?;
            Ok(result)
        }
        2 | 3 => { // LZ4, LZ4HC
            decompress(compressed, uncompressed_size)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("LZ4 decompression failed: {}", e)))
        }
        _ => Err(io::Error::new(io::ErrorKind::Unsupported, format!("Unsupported compression type: {}", compression_type))),
    }
}

// Helpers
fn read_u16_be<R: Read>(r: &mut R) -> io::Result<u16> {
    let mut b = [0u8; 2];
    r.read_exact(&mut b)?;
    Ok(u16::from_be_bytes(b))
}

fn read_u32_be<R: Read>(r: &mut R) -> io::Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_be_bytes(b))
}

fn read_u64_be<R: Read>(r: &mut R) -> io::Result<u64> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b)?;
    Ok(u64::from_be_bytes(b))
}

fn read_string<R: Read>(r: &mut R) -> io::Result<String> {
    let mut buf = Vec::new();
    loop {
        let mut b = [0u8; 1];
        r.read_exact(&mut b)?;
        if b[0] == 0 { break; }
        buf.push(b[0]);
    }
    String::from_utf8(buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
