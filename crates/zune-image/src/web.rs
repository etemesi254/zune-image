#![cfg(feature = "web")]
use crate::errors::ImageErrors;
use crate::image::Image;
use std::io::BufRead;
use std::io::{Error, ErrorKind, Read, Result as IoResult, Seek, SeekFrom};
use zune_core::options::DecoderOptions;
pub struct WebBuffer {
    /// The contiguous buffer holding downloaded data
    buffer: Vec<u8>,
    /// Our current virtual position within the stream
    position: u64,
    /// The synchronous HTTP response body reader
    stream: Box<dyn Read + Send + Sync>,
    /// The total content length (if the server provided it)
    content_length: Option<u64>,
    /// temporary buffer used to fill buffer
    temp_buffer: Vec<u8>,
}

impl WebBuffer {
    /// Create a WebBuffer from an existing network stream and pre-read header data.
    pub fn from_parts(
        stream: Box<dyn Read + Send + Sync>, initial_data: Vec<u8>, content_length: Option<u64>,
    ) -> Self {
        Self {
            buffer: initial_data,
            position: 0, // Starts at 0 so the decoder re-reads the header from RAM
            stream,
            content_length,
            temp_buffer: vec![0; 1 << 16],
        }
    }

    pub fn new(url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // Perform a synchronous GET request
        let resp = ureq::get(url).call()?;

        // Try to parse the Content-Length header to assist with SeekFrom::End
        let content_length = resp
            .headers()
            .get("Content-Length")
            .and_then(|s| s.to_str().ok()?.parse::<u64>().ok());

        let (_, body) = resp.into_parts();
        Ok(Self {
            buffer: Vec::new(),
            position: 0,
            stream: Box::new(body.into_reader()),
            temp_buffer: vec![0; 1 << 16],
            content_length,
        })
    }

    /// Helper method to fetch data until the buffer contains at least `target_pos` bytes
    fn pull_until(&mut self, target_pos: u64) -> IoResult<()> {
        while (self.buffer.len() as u64) < target_pos {
            let bytes_read = self.stream.read(&mut self.temp_buffer)?;

            if bytes_read == 0 {
                break; // We've hit EOF
            }

            self.buffer
                .extend_from_slice(&self.temp_buffer[..bytes_read]);
        }
        Ok(())
    }
}

impl Read for WebBuffer {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        // If our current position is exactly at the end of our downloaded buffer,
        // we need to pull more data from the network.
        if self.position >= self.buffer.len() as u64 {
            let target = self.position + buf.len() as u64;
            self.pull_until(target)?;
        }

        // Calculate how much data is actually available to read
        let pos = self.position as usize;
        let available = self.buffer.len().saturating_sub(pos);

        if available == 0 {
            return Ok(0); // EOF
        }

        // Copy from our internal buffer to the caller's buffer
        let to_read = std::cmp::min(buf.len(), available);
        buf[..to_read].copy_from_slice(&self.buffer[pos..pos + to_read]);

        // Advance our internal cursor
        self.position += to_read as u64;

        Ok(to_read)
    }
}

impl Seek for WebBuffer {
    fn seek(&mut self, pos: SeekFrom) -> IoResult<u64> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::Current(offset) => {
                let p = self.position as i64 + offset;
                if p < 0 {
                    return Err(Error::new(
                        ErrorKind::InvalidInput,
                        "Attempted to seek before start",
                    ));
                }
                p as u64
            }
            SeekFrom::End(offset) => {
                // To seek from the end, we ideally need to know where the end is.
                if let Some(total_len) = self.content_length {
                    let p = total_len as i64 + offset;
                    if p < 0 {
                        return Err(Error::new(
                            ErrorKind::InvalidInput,
                            "Attempted to seek before start",
                        ));
                    }
                    p as u64
                } else {
                    // If the server didn't send a Content-Length, we have no choice but
                    // to download the entire rest of the file right now to find the end.
                    let mut rest = Vec::new();
                    self.stream.read_to_end(&mut rest)?;
                    self.buffer.append(&mut rest);

                    let p = self.buffer.len() as i64 + offset;
                    if p < 0 {
                        return Err(Error::new(
                            ErrorKind::InvalidInput,
                            "Attempted to seek before start",
                        ));
                    }
                    p as u64
                }
            }
        };

        // If the decoder seeks past what we have currently downloaded, fetch until we catch up.
        self.pull_until(new_pos)?;

        // Update the position, capping it at the actual EOF if the seek was beyond the file size.
        self.position = std::cmp::min(new_pos, self.buffer.len() as u64);

        Ok(self.position)
    }
}

impl BufRead for WebBuffer {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        // If the cursor is exactly at the end of what we've downloaded, pull a bit more
        if self.position >= self.buffer.len() as u64 {
            let target = self.position + 8192; // Fetch another 8KB chunk
            self.pull_until(target)?;
        }

        // Return a slice from the current position to the end of the downloaded buffer
        let pos = self.position as usize;
        Ok(&self.buffer[pos..])
    }

    fn consume(&mut self, amt: usize) {
        // Advance the virtual cursor by the amount the decoder consumed
        self.position += amt as u64;
    }
}

pub fn open_web(url: &str, decoder_options: DecoderOptions) -> Result<Image, ImageErrors> {
    let web_buffer = WebBuffer::new(url)
        .map_err(|e| ImageErrors::GenericString(format!("Network error: {}", e)))?;

    Image::read(web_buffer, decoder_options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_web() {
        let url = "https://raw.githubusercontent.com/etemesi254/zune-image/refs/heads/dev/test-images/jpeg/2029.jpg";

        let image = open_web(url, DecoderOptions::default()).unwrap();

        assert_eq!(image.dimensions(), (388, 477));
    }
}
