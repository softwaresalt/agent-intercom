//! NDJSON codec for ACP agent streams.
//!
//! Wraps [`tokio_util::codec::LinesCodec`] with a configurable maximum line
//! length to prevent memory exhaustion caused by unterminated or maliciously
//! large messages from a misbehaving agent process.
//!
//! # Usage
//!
//! Use [`AcpCodec`] as the codec parameter for
//! [`tokio_util::codec::FramedRead`] (inbound) and
//! [`tokio_util::codec::FramedWrite`] (outbound).  Both directions enforce
//! UTF-8 line framing delimited by `\n`.

use bytes::BytesMut;
use tokio_util::codec::{Decoder, Encoder, LinesCodec, LinesCodecError};

use crate::{AppError, Result};

/// Maximum line length accepted by the ACP codec: 1 MiB.
///
/// Lines exceeding this limit on the inbound stream cause [`AcpCodec::decode`]
/// to return [`AppError::Acp`] with `"line too long"`, protecting the server
/// from allocating unbounded memory for a single message.
pub const MAX_LINE_BYTES: usize = 1_048_576;

/// NDJSON codec for bidirectional ACP agent streams.
///
/// Delegates line-framing to [`LinesCodec`] with a fixed
/// [`MAX_LINE_BYTES`] limit.  Each newline-terminated (`\n`) UTF-8 string
/// is one complete ACP message.
///
/// # Decoder
///
/// Inbound lines longer than [`MAX_LINE_BYTES`] return
/// [`AppError::Acp`]`("line too long: …")` rather than allocating.
/// I/O errors are mapped to [`AppError::Io`].
///
/// # Encoder
///
/// Outbound strings are encoded as `item\n`.  The max-length limit is a
/// decoder-side concern and is not enforced during encoding.
///
/// # Examples
///
/// ```rust,ignore
/// use tokio_util::codec::FramedRead;
/// use agent_intercom::acp::codec::AcpCodec;
///
/// let reader = FramedRead::new(child_stdout, AcpCodec::new());
/// ```
#[derive(Debug)]
pub struct AcpCodec(LinesCodec);

impl AcpCodec {
    /// Create a new `AcpCodec` with the default [`MAX_LINE_BYTES`] limit.
    #[must_use]
    pub fn new() -> Self {
        Self(LinesCodec::new_with_max_length(MAX_LINE_BYTES))
    }
}

impl Default for AcpCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder for AcpCodec {
    type Item = String;
    type Error = AppError;

    /// Decode the next newline-terminated line from `src`.
    ///
    /// Returns `Ok(None)` when `src` contains no complete line yet (buffering).
    /// Returns `Err(AppError::Acp("line too long: …"))` when the line exceeds
    /// [`MAX_LINE_BYTES`].
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>> {
        self.0.decode(src).map_err(map_codec_error)
    }

    /// Decode the final line when the stream reaches EOF.
    ///
    /// Delegates to [`LinesCodec::decode_eof`], applying the same error mapping.
    fn decode_eof(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>> {
        self.0.decode_eof(src).map_err(map_codec_error)
    }
}

impl Encoder<String> for AcpCodec {
    type Error = AppError;

    /// Encode `item` as a `\n`-terminated NDJSON line into `dst`.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::Io`] on underlying I/O failures.
    fn encode(&mut self, item: String, dst: &mut BytesMut) -> Result<()> {
        // LinesCodec::encode does not enforce a max line length;
        // the limit applies only to decoding.
        self.0.encode(item, dst).map_err(map_codec_error)
    }
}

// ── Private helper ────────────────────────────────────────────────────────────

/// Map a [`LinesCodecError`] to an [`AppError`].
fn map_codec_error(e: LinesCodecError) -> AppError {
    match e {
        LinesCodecError::MaxLineLengthExceeded => {
            AppError::Acp(format!("line too long: exceeded {MAX_LINE_BYTES} bytes"))
        }
        LinesCodecError::Io(io_err) => AppError::Io(io_err.to_string()),
    }
}
