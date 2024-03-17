use std::fmt::{LowerHex, Octal};
use std::fs::Metadata;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

use super::core::TarChunk;
use super::error::{TarError, TarResult};

pub struct TarHeader {
    path: String,
    data: Box<[u8; 512]>,
}

impl TarHeader {
    pub fn empty(path: String) -> Self {
        Self {
            path: path,
            data: Box::new([0; 512]),
        }
    }

    fn slice(header: &mut [u8; 512], offset: usize, length: usize) -> TarResult<&mut [u8]> {
        match header.get_mut(offset..offset + length) {
            Some(data) => Ok(data),
            None => Err(TarError::memory_access(format!(
                "Header cannot be sliced with ({}, {})",
                offset,
                offset + length
            ))),
        }
    }

    fn write_octal<T: LowerHex + Octal>(
        header: &mut [u8; 512],
        offset: usize,
        length: usize,
        value: T,
    ) -> TarResult<()> {
        let data = Self::slice(header, offset, length)?;
        let value = format!("{:011o}", value);
        let value = value.as_bytes();

        for i in 0..(data.len() - 1) {
            let index = data.len() - 2 - i;
            let target = match data.get_mut(index) {
                Some(target) => target,
                None => {
                    return Err(TarError::memory_access(format!(
                        "Header cannot be accessed at {} within ({}, {})",
                        index,
                        offset,
                        offset + length
                    )))
                }
            };

            *target = match value.get(value.len() - i - 1) {
                Some(value) => *value,
                None => b'0',
            };
        }

        match data.get_mut(data.len() - 1) {
            Some(target) => *target = b'\0',
            None => {
                return Err(TarError::memory_access(format!(
                    "Header cannot be accessed at {} within ({}, {})",
                    data.len() - 1,
                    offset,
                    offset + length
                )))
            }
        };

        Ok(())
    }

    fn write_bytes(header: &mut [u8; 512], offset: usize, length: usize, bytes: &[u8]) -> TarResult<()> {
        let data = Self::slice(header, offset, length)?;

        for i in 0..data.len() {
            let target = match data.get_mut(i) {
                Some(value) => value,
                None => {
                    return Err(TarError::memory_access(format!(
                        "Header cannot be accessed at {} within ({}, {})",
                        i,
                        offset,
                        offset + length
                    )))
                }
            };

            *target = match bytes.get(i) {
                Some(value) => *value,
                None => b'\0',
            };
        }

        Ok(())
    }

    fn write_name(header: &mut [u8; 512], path: &str) -> TarResult<()> {
        Self::write_bytes(header, 0, 99, path.as_bytes())
    }

    fn write_mode(header: &mut [u8; 512], metadata: &Metadata) -> TarResult<()> {
        Self::write_octal(header, 100, 8, metadata.permissions().mode() & 0o777)
    }

    fn write_uid(header: &mut [u8; 512], uid: u32) -> TarResult<()> {
        Self::write_octal(header, 108, 8, uid)
    }

    fn write_gid(header: &mut [u8; 512], gid: u32) -> TarResult<()> {
        Self::write_octal(header, 116, 8, gid)
    }

    fn write_size(header: &mut [u8; 512], metadata: &Metadata) -> TarResult<()> {
        Self::write_octal(header, 124, 12, metadata.size())
    }

    fn write_mtime(header: &mut [u8; 512], metadata: &Metadata) -> TarResult<()> {
        Self::write_octal(header, 136, 12, metadata.mtime())
    }

    fn write_chksum(header: &mut [u8; 512]) -> TarResult<()> {
        Self::write_bytes(header, 148, 8, b"        ")?;
        Self::write_octal(header, 148, 8, Self::calculate_checksum(header))
    }

    fn write_type_flag(header: &mut [u8; 512]) -> TarResult<()> {
        Self::write_bytes(header, 156, 1, b"0")
    }

    fn write_magic(header: &mut [u8; 512]) -> TarResult<()> {
        Self::write_bytes(header, 257, 8, b"ustar  \0")
    }

    fn calculate_checksum(header: &[u8; 512]) -> u32 {
        let mut checksum: u32 = 0;

        for i in header.iter() {
            checksum += *i as u32;
        }

        checksum
    }

    pub fn write(mut self, metadata: &Metadata) -> TarResult<TarChunk> {
        let data = &mut self.data;

        Self::write_name(data, &self.path)?;
        Self::write_mode(data, metadata)?;
        Self::write_uid(data, 0)?;
        Self::write_gid(data, 0)?;
        Self::write_size(data, metadata)?;
        Self::write_mtime(data, metadata)?;
        Self::write_magic(data)?;
        Self::write_type_flag(data)?;
        Self::write_chksum(data)?;

        Ok(self.into())
    }
}

impl Into<TarChunk> for TarHeader {
    fn into(self) -> TarChunk {
        TarChunk::header(self.path, self.data)
    }
}
