use std::cmp;
use std::fmt;
use std::io;
use std::io::BufRead;
use std::io::Read;

pub fn decoder_helper(decoder: &mut encoding_rs::Decoder, input: &[u8]) -> io::Result<String> {
    let mut decoded = String::with_capacity(input.len() * 4);

    let (result, bytes_read) =
        decoder.decode_to_string_without_replacement(&input, &mut decoded, false);
    if let encoding_rs::DecoderResult::Malformed(_, _) = result {
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("Malformed input. {:x?}, position {}.", input, bytes_read),
        ))
    } else {
        Ok(decoded)
    }
}

pub struct CodecReadBuffer<R> {
    inner: R,
    decoder: encoding_rs::Decoder,
    input_buf: Vec<u8>,
    output_buf: String,
    output_pos: usize,
}

impl<R: Read> CodecReadBuffer<R> {
    /// Create a re-encoding buffered reader for the provided reader, and the specified encoding
    pub fn for_encoding(inner: R, encoding_name: &str) -> io::Result<Self> {
        Self::for_encoding_with_capacity(inner, encoding_name, ::DEFAULT_BUF_SIZE)
    }

    /// Create a re-encoding buffered reading with the specified buffer capacity
    pub fn for_encoding_with_capacity(
        inner: R,
        encoding_name: &str,
        capacity: usize,
    ) -> io::Result<Self> {
        Self::for_encoding_with_initial_buffer(inner, encoding_name, Vec::with_capacity(capacity))
    }

    pub fn for_encoding_with_initial_buffer(
        inner: R,
        encoding_name: &str,
        input_buf: Vec<u8>,
    ) -> io::Result<Self> {
        let decoder = encoding_rs::Encoding::for_label_no_replacement(&encoding_name.as_bytes())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::Other,
                    format!("Unrecognized input encoding name: {}", encoding_name),
                )
            }).map(|enc| enc.new_decoder_without_bom_handling())?;

        Ok(CodecReadBuffer {
            inner,
            decoder,
            input_buf,
            output_buf: String::new(),
            output_pos: 0,
        })
    }

    fn fill_input_buf(&mut self) -> io::Result<usize> {
        if self.input_buf.is_empty() {
            let capacity = self.input_buf.capacity();
            // Read::read() ignores capacity, and reads from the beginning
            // of the used space to the end of the used space
            // So we need to force our input_buf's len() up to match capacity()
            if self.input_buf.len() < capacity {
                self.input_buf.resize(capacity, 0);
            }
            let read_size = self.inner.read(&mut self.input_buf)?;
            // The read may not have filled the buffer we gave it, so we need
            // to resize the buffer from capacity() down to read_size
            self.input_buf.resize(read_size, 0);
            Ok(read_size)
        } else {
            Ok(0)
        }
    }
}

impl<R: Read> Read for CodecReadBuffer<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let nread = {
            let mut rem = self.fill_buf()?;
            rem.read(buf)?
        };

        self.consume(nread);
        Ok(nread)
    }
}

impl<R: Read> BufRead for CodecReadBuffer<R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.output_pos >= self.output_buf.len() {
            debug_assert!(self.output_pos == self.output_buf.len());
            self.fill_input_buf()?;
            // Take raw encoded data and convert it to utf-8
            self.output_buf =
                decoder_helper(&mut self.decoder, &self.input_buf).map_err(|desc| {
                    io::Error::new(
                        io::ErrorKind::Other,
                        format!("Input decoding error: {}", desc),
                    )
                })?;
            self.input_buf.clear();
            self.output_pos = 0;
        }
        Ok(&self.output_buf.as_bytes()[self.output_pos..])
    }

    fn consume(&mut self, amt: usize) {
        self.output_pos = cmp::min(self.output_pos + amt, self.output_buf.len());
    }
}

impl<R> fmt::Debug for CodecReadBuffer<R>
where
    R: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("CodecReadBuffer")
            .field("reader", &self.inner)
            .field(
                "output_buf",
                &format_args!(
                    "{}/{}",
                    self.output_buf.len() - self.output_pos,
                    self.output_buf.len()
                ),
            ).finish()
    }
}

#[cfg(test)]
mod reader_tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn test_utf8() {
        // Validation docs have the same text as the test docs, but always in utf-8, no bom
        let utf8_validation = include_bytes!("../tests/validation/utf8.xml").to_vec();

        let utf8_bytes = include_bytes!("../tests/utf8/doc.xml").to_vec();
        match CodecReadBuffer::for_encoding(&utf8_bytes as &[u8], "utf-8") {
            Ok(mut decoding_reader) => {
                let mut utf8_encoded_doc: String = String::new();
                decoding_reader
                    .read_to_string(&mut utf8_encoded_doc)
                    .expect("Failed decoding input data");
                assert_eq!(&utf8_validation, &utf8_encoded_doc.as_bytes());
            }
            Err(e) => panic!("Failed initializing CodecReadBuffer: {}", e),
        }
    }

    #[test]
    fn test_utf8_with_bom() {
        // Validation docs have the same text as the test docs, but always in utf-8, no bom
        let utf8_validation = include_bytes!("../tests/validation/utf8.xml").to_vec();

        let utf8_with_bom_bytes = include_bytes!("../tests/utf8_bom/doc.xml").to_vec();
        match CodecReadBuffer::for_encoding(&utf8_with_bom_bytes as &[u8], "utf-8") {
            Ok(mut decoding_reader) => {
                let mut utf8_encoded_doc: String = String::new();
                decoding_reader
                    .read_to_string(&mut utf8_encoded_doc)
                    .expect("Failed decoding input data");
                assert_eq!(&utf8_validation[..], &utf8_encoded_doc.as_bytes()[3..]);
            }
            Err(e) => panic!("Failed initializing CodecReadBuffer: {}", e),
        }
    }

    #[test]
    fn test_utf16le() {
        // Validation docs have the same text as the test docs, but always in utf-8, no bom
        let utf16_validation = include_bytes!("../tests/validation/utf16le.xml").to_vec();

        let utf16_bytes = include_bytes!("../tests/utf16le/doc.xml").to_vec();
        match CodecReadBuffer::for_encoding(&utf16_bytes as &[u8], "utf-16le") {
            Ok(mut decoding_reader) => {
                let mut utf8_encoded_doc: String = String::new();
                decoding_reader
                    .read_to_string(&mut utf8_encoded_doc)
                    .expect("Failed decoding input data");
                assert_eq!(&utf16_validation, &utf8_encoded_doc.as_bytes());
            }
            Err(e) => panic!("Failed initializing CodecReadBuffer: {}", e),
        }
    }

    #[test]
    fn test_utf16le_with_bom() {
        // Validation docs have the same text as the test docs, but always in utf-8, no bom
        let utf16_validation = include_bytes!("../tests/validation/utf16le.xml").to_vec();

        let utf16_with_bom_bytes = include_bytes!("../tests/utf16le_bom/doc.xml").to_vec();
        match CodecReadBuffer::for_encoding(&utf16_with_bom_bytes as &[u8], "utf-16le") {
            Ok(mut decoding_reader) => {
                let mut utf8_encoded_doc: String = String::new();
                decoding_reader
                    .read_to_string(&mut utf8_encoded_doc)
                    .expect("Failed decoding input data");
                assert_eq!(&utf16_validation[..], &utf8_encoded_doc.as_bytes()[3..]);
            }
            Err(e) => panic!("Failed initializing CodecReadBuffer: {}", e),
        }
    }

    #[test]
    fn test_utf16be() {
        // Validation docs have the same text as the test docs, but always in utf-8, no bom
        let utf16_validation = include_bytes!("../tests/validation/utf16be.xml").to_vec();

        let utf16_bytes = include_bytes!("../tests/utf16be/doc.xml").to_vec();
        match CodecReadBuffer::for_encoding(&utf16_bytes as &[u8], "utf-16be") {
            Ok(mut decoding_reader) => {
                let mut utf8_encoded_doc: String = String::new();
                decoding_reader
                    .read_to_string(&mut utf8_encoded_doc)
                    .expect("Failed decoding input data");
                assert_eq!(&utf16_validation, &utf8_encoded_doc.as_bytes());
            }
            Err(e) => panic!("Failed initializing CodecReadBuffer: {}", e),
        }
    }

    #[test]
    fn test_utf16be_with_bom() {
        // Validation docs have the same text as the test docs, but always in utf-8, no bom
        let utf16_validation = include_bytes!("../tests/validation/utf16be.xml").to_vec();

        let utf16_with_bom_bytes = include_bytes!("../tests/utf16be_bom/doc.xml").to_vec();
        match CodecReadBuffer::for_encoding(&utf16_with_bom_bytes as &[u8], "utf-16be") {
            Ok(mut decoding_reader) => {
                let mut utf8_encoded_doc: String = String::new();
                decoding_reader
                    .read_to_string(&mut utf8_encoded_doc)
                    .expect("Failed decoding input data");
                assert_eq!(&utf16_validation[..], &utf8_encoded_doc.as_bytes()[3..]);
            }
            Err(e) => panic!("Failed initializing CodecReadBuffer: {}", e),
        }
    }
}
