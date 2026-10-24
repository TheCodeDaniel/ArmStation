//! Bounds-checked little-endian cursor. Every read returns `Err` instead of panicking.

use crate::error::{LoadError, Result};

#[derive(Debug)]
pub(crate) struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    what: &'static str,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8], offset: u64, what: &'static str) -> Result<Self> {
        let pos = usize::try_from(offset).map_err(|_| LoadError::OutOfBounds { what })?;
        if pos > data.len() {
            return Err(LoadError::Truncated {
                what,
                offset,
                needed: 0,
                len: data.len() as u64,
            });
        }
        Ok(Self { data, pos, what })
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|e| *e <= self.data.len())
            .ok_or(LoadError::Truncated {
                what: self.what,
                offset: self.pos as u64,
                needed: n as u64,
                len: self.data.len() as u64,
            })?;
        let s = &self.data[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    pub fn bytes<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut a = [0u8; N];
        a.copy_from_slice(self.take(N)?);
        Ok(a)
    }

    pub fn skip(&mut self, n: usize) -> Result<()> {
        self.take(n).map(|_| ())
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes::<1>()?[0])
    }
    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.bytes()?))
    }
    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.bytes()?))
    }
    pub fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.bytes()?))
    }
}
