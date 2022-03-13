use std::fs;
use std::io;
use std::io::Write;
use std::path::PathBuf;

pub struct Split {
    num: usize,             // maximum size of each split
    pos: usize,             // written bytes of current split
    tot: u64,               // total bytes written
    val: u64,               // number of file splits
    prefix: PathBuf,        // path prefix
    file: Option<fs::File>, // current output file
    mark_failed: bool,      // Split had an error
}

impl Drop for Split {
    fn drop(&mut self) {
        log::trace!(
            "Split statistics: prefix={prefix:?} total_bytes={total_bytes} chunks={chunks} failed={failed}",
            prefix=self.prefix,
            total_bytes=self.tot,
            chunks=self.val,
            failed=self.mark_failed
        );
        if let Err(err) = self.flush() {
            let path_buf = self.current_path_buf();
            log::error!(
                "Cannot flush {path}: {err}",
                path = path_buf.as_path().display()
            );
        }
    }
}

impl Split {
    pub fn new(prefix: PathBuf, num: usize) -> Self {
        Split {
            num,
            pos: 0,
            tot: 0,
            val: 0,
            prefix,
            file: None,
            mark_failed: false,
        }
    }

    pub fn clear(&mut self) -> io::Result<()> {
        self.pos = 0;
        self.tot = 0;
        self.val = 0;
        let result = self.flush();
        self.file = None;
        self.mark_failed = false;
        result
    }

    pub fn written(&self) -> u64 {
        self.tot
    }

    fn current_path_buf(&mut self) -> PathBuf {
        self.prefix
            .join(format!("chunk.{chunk_seq}", chunk_seq = self.val))
    }

    fn use_file_or_next(&mut self) -> io::Result<usize> {
        assert!(self.pos <= self.num, "file position exceeded max size");

        if self.mark_failed {
            log::error!(
                "Split is marked failed at {total_bytes}",
                total_bytes = self.tot
            );
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "Split is marked failed at {total_bytes}",
                    total_bytes = self.tot
                ),
            ));
        }

        if self.file.is_some() && self.pos < self.num {
            return Ok(self.num - self.pos);
        }

        self.val += 1;
        let path_buf = self.current_path_buf();
        let file_path = path_buf.as_path();

        log::trace!("Creating new chunk {file_path:?}");

        self.file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(file_path)
        {
            Ok(file) => Some(file),
            Err(err) => {
                self.mark_failed = true;

                log::error!(
                    "Cannot create new file {path}, marking Split failed",
                    path = file_path.display()
                );

                return Err(io::Error::new(
                    err.kind(),
                    format!("Cannot create new file {path}", path = file_path.display()),
                ));
            }
        };
        self.pos = 0;

        Ok(self.num)
    }

    fn write_once(&mut self, buf: &[u8]) -> io::Result<usize> {
        let buf_len = buf.len();
        if buf_len == 0 {
            return Ok(0);
        }
        assert!(buf_len <= self.num, "buffer too large");

        if self.mark_failed {
            log::error!(
                "Split failed at position {total_bytes}, ignoring write request",
                total_bytes = self.tot
            );
            return Ok(0);
        }

        self.use_file_or_next()?;

        let file = self.file.as_ref();

        let mut slice = buf;
        let n = io::copy(&mut slice, &mut file.unwrap())?;

        let offset = usize::try_from(n).expect("copied buffer exceeds usize");

        self.tot += n;
        self.pos += offset;
        assert!(self.pos <= self.num, "Split.pos > Split.num");

        Ok(offset)
    }
}

impl io::Write for Split {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.mark_failed {
            log::error!(
                "Ignoring error at position {total_bytes}",
                total_bytes = self.tot
            );
            return Ok(0);
        }

        let buf_len = buf.len();
        let mut written = 0;

        let remainder = self.num - self.pos;

        let (left, right) = if remainder < buf_len {
            // buf_len >= remainder: split buf into buf[0..remainder-1], buf[remainder..]
            buf.split_at(remainder)
        } else {
            // buf_len < remainder: split buf into buf[0..buf_len-1] (buf), buf[buf_len..] ([])
            buf.split_at(buf_len)
        };

        // write left slice of length remainder or buf_len
        written += self.write_once(left)?;

        // write right slice in chunks of length self.num (last chunk at most self.num)
        for chunk in right.chunks(self.num) {
            written += self.write_once(chunk)?;
        }

        assert_eq!(buf_len, written, "buf.len() != written");

        Ok(written)
    }

    #[inline]
    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        let total_len = bufs.iter().map(|buf| self.write(buf).unwrap()).sum();
        Ok(total_len)
    }

    // #[inline]
    // fn is_write_vectored(&self) -> bool {
    //     true
    // }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        if self.file.is_some() {
            return self.file.as_ref().unwrap().flush();
        }
        Ok(())
    }
}
