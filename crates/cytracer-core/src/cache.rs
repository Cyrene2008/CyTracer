use std::io::{Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};

pub const MAGIC: &[u8; 4] = b"CYMC";
pub const VERSION: u16 = 1;

/// 指标缓存头部（同时作为缓存复用指纹）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheHeader {
    pub fps: f32,
    pub width: u16,
    pub height: u16,
    pub block: u16,
    pub compensation: bool,
    pub frame_count: u32,
    pub duration: f32,
    pub source_path: String,
    pub source_size: u64,
    pub source_mtime: i64,
}

impl CacheHeader {
    pub fn matches(&self, fps: f32, compensation: bool, width: u16, height: u16, block: u16) -> bool {
        (self.fps - fps).abs() < 0.01
            && self.compensation == compensation
            && self.width == width
            && self.height == height
            && self.block == block
    }
}

/// 缓存指纹：路径 + 体积 + 修改时间，保证源文件变化后缓存失效。
pub fn cache_key(path: &Path) -> (String, u64, i64) {
    let meta = std::fs::metadata(path).ok();
    let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let mtime = meta
        .as_ref()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let canonical = std::fs::canonicalize(path)
        .map(|p| p.to_string_lossy().to_lowercase())
        .unwrap_or_else(|_| path.to_string_lossy().to_lowercase());
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in canonical.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    for byte in size.to_le_bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    for byte in mtime.to_le_bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    (format!("{hash:016x}"), size, mtime)
}

pub struct CacheWriter {
    file: std::io::BufWriter<std::fs::File>,
    tmp_path: std::path::PathBuf,
    final_path: std::path::PathBuf,
    header: CacheHeader,
    frames_written: u32,
}

impl CacheWriter {
    pub fn create(final_path: &Path, header: CacheHeader) -> Result<Self> {
        let tmp_path = final_path.with_extension("cymc.tmp");
        if let Some(parent) = final_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::File::create(&tmp_path)?;
        let mut writer = Self {
            file: std::io::BufWriter::new(file),
            tmp_path,
            final_path: final_path.to_path_buf(),
            header,
            frames_written: 0,
        };
        writer.write_header_placeholder()?;
        Ok(writer)
    }

    fn write_header_placeholder(&mut self) -> Result<()> {
        let path_bytes = self.header.source_path.as_bytes();
        let path_len = path_bytes.len().min(u16::MAX as usize) as u16;
        self.file.write_all(MAGIC)?;
        self.file.write_all(&VERSION.to_le_bytes())?;
        self.file.write_all(&self.header.fps.to_le_bytes())?;
        self.file.write_all(&self.header.width.to_le_bytes())?;
        self.file.write_all(&self.header.height.to_le_bytes())?;
        self.file.write_all(&self.header.block.to_le_bytes())?;
        self.file.write_all(&[self.header.compensation as u8])?;
        self.file.write_all(&self.header.frame_count.to_le_bytes())?;
        self.file.write_all(&self.header.duration.to_le_bytes())?;
        self.file.write_all(&self.header.source_size.to_le_bytes())?;
        self.file.write_all(&self.header.source_mtime.to_le_bytes())?;
        self.file.write_all(&path_len.to_le_bytes())?;
        self.file.write_all(&path_bytes[..path_len as usize])?;
        Ok(())
    }

    pub fn push(&mut self, metric: &crate::types::FrameMetric) -> Result<()> {
        self.file.write_all(&metric.mean_diff.to_le_bytes())?;
        self.file.write_all(&metric.median_diff.to_le_bytes())?;
        self.file.write_all(&metric.max_diff.to_le_bytes())?;
        self.file.write_all(&[metric.scene_cut as u8])?;
        for value in metric.bbox {
            self.file.write_all(&value.to_le_bytes())?;
        }
        for value in metric.hist {
            self.file.write_all(&value.to_le_bytes())?;
        }
        self.frames_written += 1;
        Ok(())
    }

    /// 回填帧数并原子重命名。
    pub fn finish(mut self) -> Result<CacheHeader> {
        self.header.frame_count = self.frames_written;
        self.file.flush()?;
        let mut file = self
            .file
            .into_inner()
            .map_err(|err| CoreError::Cache(err.to_string()))?;
        use std::io::{Seek, SeekFrom};
        file.seek(SeekFrom::Start(0))?;
        // 重写完整头部
        let mut header_bytes: Vec<u8> = Vec::new();
        let path_bytes = self.header.source_path.as_bytes();
        let path_len = path_bytes.len().min(u16::MAX as usize) as u16;
        header_bytes.extend_from_slice(MAGIC);
        header_bytes.extend_from_slice(&VERSION.to_le_bytes());
        header_bytes.extend_from_slice(&self.header.fps.to_le_bytes());
        header_bytes.extend_from_slice(&self.header.width.to_le_bytes());
        header_bytes.extend_from_slice(&self.header.height.to_le_bytes());
        header_bytes.extend_from_slice(&self.header.block.to_le_bytes());
        header_bytes.push(self.header.compensation as u8);
        header_bytes.extend_from_slice(&self.header.frame_count.to_le_bytes());
        header_bytes.extend_from_slice(&self.header.duration.to_le_bytes());
        header_bytes.extend_from_slice(&self.header.source_size.to_le_bytes());
        header_bytes.extend_from_slice(&self.header.source_mtime.to_le_bytes());
        header_bytes.extend_from_slice(&path_len.to_le_bytes());
        header_bytes.extend_from_slice(&path_bytes[..path_len as usize]);
        file.write_all(&header_bytes)?;
        file.sync_all()?;
        drop(file);
        if self.final_path.exists() {
            let _ = std::fs::remove_file(&self.final_path);
        }
        std::fs::rename(&self.tmp_path, &self.final_path)?;
        Ok(self.header)
    }

    pub fn abandon(&mut self) {
        let _ = std::fs::remove_file(&self.tmp_path);
    }
}

pub struct CacheReader {
    pub header: CacheHeader,
    reader: std::io::BufReader<std::fs::File>,
}

impl CacheReader {
    pub fn open(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        let mut reader = std::io::BufReader::new(file);
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if &magic != MAGIC {
            return Err(CoreError::Cache("bad magic".into()));
        }
        let version = read_u16(&mut reader)?;
        if version != VERSION {
            return Err(CoreError::Cache(format!("unsupported version {version}")));
        }
        let fps = read_f32(&mut reader)?;
        let width = read_u16(&mut reader)?;
        let height = read_u16(&mut reader)?;
        let block = read_u16(&mut reader)?;
        let mut flag = [0u8; 1];
        reader.read_exact(&mut flag)?;
        let frame_count = read_u32(&mut reader)?;
        let duration = read_f32(&mut reader)?;
        let source_size = read_u64(&mut reader)?;
        let source_mtime = read_i64(&mut reader)?;
        let path_len = read_u16(&mut reader)? as usize;
        let mut path_bytes = vec![0u8; path_len];
        reader.read_exact(&mut path_bytes)?;
        let header = CacheHeader {
            fps,
            width,
            height,
            block,
            compensation: flag[0] != 0,
            frame_count,
            duration,
            source_path: String::from_utf8_lossy(&path_bytes).to_string(),
            source_size,
            source_mtime,
        };
        Ok(Self { header, reader })
    }

    pub fn read_metric(&mut self) -> Result<Option<crate::types::FrameMetric>> {
        let mut buf = [0u8; 4];
        match self.reader.read_exact(&mut buf) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(err) => return Err(err.into()),
        }
        let mean_diff = f32::from_le_bytes(buf);
        let median_diff = read_f32(&mut self.reader)?;
        let max_diff = read_f32(&mut self.reader)?;
        let mut scene = [0u8; 1];
        self.reader.read_exact(&mut scene)?;
        let mut bbox = [0u16; 4];
        for value in bbox.iter_mut() {
            *value = read_u16(&mut self.reader)?;
        }
        let mut hist = [0u16; 16];
        for value in hist.iter_mut() {
            *value = read_u16(&mut self.reader)?;
        }
        Ok(Some(crate::types::FrameMetric {
            mean_diff,
            median_diff,
            max_diff,
            scene_cut: scene[0] != 0,
            bbox,
            hist,
        }))
    }

    pub fn read_all(&mut self) -> Result<Vec<crate::types::FrameMetric>> {
        let mut metrics = Vec::with_capacity(self.header.frame_count as usize);
        while let Some(metric) = self.read_metric()? {
            metrics.push(metric);
        }
        Ok(metrics)
    }
}

fn read_u16(reader: &mut impl Read) -> Result<u16> {
    let mut buf = [0u8; 2];
    reader.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}
fn read_u32(reader: &mut impl Read) -> Result<u32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}
fn read_u64(reader: &mut impl Read) -> Result<u64> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf))
}
fn read_i64(reader: &mut impl Read) -> Result<i64> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf)?;
    Ok(i64::from_le_bytes(buf))
}
fn read_f32(reader: &mut impl Read) -> Result<f32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(f32::from_le_bytes(buf))
}
