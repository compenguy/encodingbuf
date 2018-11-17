/*!
This crate provides a reader that transcodes from arbitrary text encodings
into rust-friendly UTF-8, and a writer that transcodes from rust-friendly
UTF-8 into arbitrary text encodings.

# Examples

```rust
use recoder::EncodingToUtf8Reader;
let utf8_doc = "<note>
<to>Tove</to>
<from>Jani</from>
<heading>Reminder</heading>
<body>Don't forget me this weekend!</body>
</note>";

let mut reader = EncodingToUtf8Reader::new(utf8_doc.as_bytes()).expect("Failed initializing recoder reader");
let mut utf8_recoded: String = String::new();
utf8_recoder.read_to_string(&mut utf8_recoded).expect("Failed reading recoded data");
assert_eq!(utf8_doc, utf8_recoded);
```
*/

#![deny(missing_docs)]

extern crate chardet;
extern crate encoding;

use encoding::types::EncodingRef;

use std::io;
use std::io::Read;
use std::io::BufRead;
use std::cmp;
use std::fmt;

const DEFAULT_BUF_SIZE: usize = 4096;

/// The `EncodingToUtf8Reader` struct adds buffered, text transcoding into UTF-8
/// to any reader.
///
///
/// # Examples
///
/// ```rust
/// use recoder::EncodingToUtf8Reader;
/// let utf8_doc = "<note>
/// <to>Tove</to>
/// <from>Jani</from>
/// <heading>Reminder</heading>
/// <body>Don't forget me this weekend!</body>
/// </note>";
/// 
/// let mut reader = EncodingToUtf8Reader::new(utf8_doc.as_bytes()).expect("Failed initializing recoder reader");
/// let mut utf8_recoded: String = String::new();
/// utf8_recoder.read_to_string(&mut utf8_recoded).expect("Failed reading recoded data");
/// assert_eq!(utf8_doc, utf8_recoded);
/// ```
pub struct EncodingToUtf8Reader<R> {
    inner: R,
    codec: EncodingRef,
    output_buf: String,
    input_buf: Vec<u8>,
    pos: usize,
}

impl<R:Read> EncodingToUtf8Reader<R> {
    /// Trivial constructor using default value for size of the read buffer,
    /// as well as automatically detecting the input encoding.
    pub fn new(inner: R) -> io::Result<Self> {
        Self::with_capacity(DEFAULT_BUF_SIZE, inner)
    }

    /// Constructor allowing configuration of the size of the read buffer,
    /// as well as automatically detecting the input encoding.
    pub fn with_capacity(capacity: usize, mut inner: R) -> io::Result<Self> {
        // pre-read from the reader so we have some content on which to
        // detect the encoding.
        // We'll init the input buf with the detection buffer so as to not
        // lose that data
        let mut detection_buf: Vec<u8> = Vec::with_capacity(capacity);
        inner.read(&mut detection_buf)?;
        let encoding_name = chardet::detect(&detection_buf[0..std::cmp::min(64,detection_buf.len())]).0;
        let codec = Self::get_encodingref(&encoding_name)
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Failed input encoding detection"))?;

        Ok(Self::with_codec_and_input_buf(codec, detection_buf, inner))
    }

    /// Constructor allowing caller to choose the input encoding, as well as
    /// set the read buffer capacity.
    pub fn with_input_encoding_and_capacity(encoding_name: &str, capacity: usize, inner: R) -> io::Result<Self> {
        let codec = Self::get_encodingref(&encoding_name)
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Unrecognized input encoding name"))?;
        Ok(Self::with_codec_and_input_buf(codec, Vec::with_capacity(capacity), inner))
    }

    fn with_codec_and_input_buf(codec: EncodingRef, input_buf: Vec<u8>, inner: R) -> Self {
        EncodingToUtf8Reader {
            inner,
            codec,
            output_buf: String::new(),
            pos: 0,
            input_buf
        }
    }

    fn get_encodingref(name: &str) -> Option<EncodingRef> {
        let encoding_name = String::from(name);
        encoding::label::encoding_from_whatwg_label(chardet::charset2encoding(&encoding_name))
    }

    fn fill_input_buf(&mut self) -> io::Result<usize> {
        if self.input_buf.is_empty() {
            self.inner.read(&mut self.input_buf)
        } else {
            Ok(0)
        }
    }
}

impl<R: Read> Read for EncodingToUtf8Reader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let nread = {
            let mut rem = self.fill_buf()?;
            rem.read(buf)?
        };

        self.consume(nread);
        Ok(nread)
    }
}

impl<R: Read> BufRead for EncodingToUtf8Reader<R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.pos >= self.output_buf.len() {
            debug_assert!(self.pos == self.output_buf.len());
            self.fill_input_buf()?;
            self.output_buf = self.codec
                .decode(&self.input_buf, encoding::DecoderTrap::Ignore)
                .expect("Input encoding error");
            self.input_buf.clear();
        }
        Ok(&self.output_buf.as_bytes()[self.pos..])
    }

    fn consume(&mut self, amt: usize) {
        self.pos = cmp::min(self.pos + amt, self.output_buf.len());
    }
}

impl<R> fmt::Debug for EncodingToUtf8Reader<R> where R: fmt::Debug {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("EncodingToUtf8Reader")
            .field("reader", &self.inner)
            .field("output_buf", &format_args!("{}/{}", self.output_buf.len() - self.pos, self.output_buf.len()))
            .finish()
    }
}

#[cfg(test)]
mod reader_tests {
    use super::*;
    use encoding::Encoding;
    use std::io::Read;

    #[test]
    fn test_utf8() {
        let utf8_doc = "<note>
<to>Tove</to>
<from>Jani</from>
<heading>Reminder</heading>
<body>Don't forget me this weekend!</body>
</note>";
        let mut utf8_recoder = EncodingToUtf8Reader::new(utf8_doc.as_bytes()).expect("Failed initializing recoder reader");
        let mut utf8_recoded: String = String::new();
        utf8_recoder.read_to_string(&mut utf8_recoded).expect("Failed reading recoded data");
        assert_eq!(utf8_doc, utf8_recoded);
    }

    #[test]
    fn test_utf16() {
        let utf8_doc = "<note>
<to>Tove</to>
<from>Jani</from>
<heading>Reminder</heading>
<body>Don't forget me this weekend!</body>
</note>";
        let utf16_doc = encoding::all::UTF_16LE.encode(utf8_doc, encoding::types::EncoderTrap::Strict).ok().unwrap();

        let mut utf16_recoder = EncodingToUtf8Reader::new(utf16_doc.as_slice()).expect("Failed initializing recoder reader");
        let mut utf16_recoded: String = String::new();
        utf16_recoder.read_to_string(&mut utf16_recoded).expect("Failed reading recoded data");
        assert_eq!(utf8_doc, utf16_recoded);
    }
}

