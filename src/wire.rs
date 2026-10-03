//! Encoding, in one place.
//!
//! Everything autobahn writes in binary goes through here: control and
//! transport frames, the ancestor journal and its checkpoints, the scan
//! cache. One module so the format is stated once and the decode limits
//! are a policy rather than a habit.
//!
//! The format is postcard: varint lengths, no padding, no self
//! description. It is smaller than the fixed-width encoding this used
//! to write — a generation counter under 128 costs one byte instead of
//! eight — and, unlike bincode, it is maintained. bincode 1.3 carried
//! RUSTSEC-2025-0141; bincode 3.0.0 is a `compile_error!` announcing
//! that the crate, 2.x included, is unmaintained.
//!
//! Nothing readable was left behind, because nothing was released: the
//! change landed before the first tag, so no journal anywhere holds the
//! old bytes. The checkpoint format still rises for it, which is what
//! refuses one written by a development build rather than misreading
//! it.
//!
//! The ceiling means something different here. Fixed-width decoders
//! read a length and allocate it, so a frame claiming four gigabytes
//! got four gigabytes before anything checked, and the cap had to sit
//! inside the decoder. postcard reads varints into serde's own
//! sequence building, which grows with what actually arrives rather
//! than with what was claimed — so the bound that matters is the
//! length of the input, and [`decode_capped`] refuses past it before
//! decoding begins.

use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Serialize};

/// The most a message that arrived from somewhere else may be.
///
/// Above any frame the transport sends — it caps frames itself, and a
/// message too large for one is split and reassembled — and far below
/// the point where a lie about a length costs anything.
pub const CEILING: usize = 256 * 1024 * 1024;

/// Encodes a value.
pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    postcard::to_allocvec(value).context("unable to encode")
}

/// Encodes a value into a writer, for a journal appending a record
/// rather than building one in memory first.
pub fn encode_into<T: Serialize, W: std::io::Write>(value: &T, writer: &mut W) -> Result<usize> {
    let mut counted = Counting {
        inner: writer,
        written: 0,
    };
    postcard::to_io(value, &mut counted).context("unable to encode")?;
    Ok(counted.written as usize)
}

/// What a value will take, without keeping it.
pub fn size_of<T: Serialize>(value: &T) -> Result<u64> {
    // postcard can bound a type's size at compile time but not measure
    // a value's, so this encodes into a sink: it costs the encoding and
    // allocates nothing.
    let mut counted = Counting {
        inner: std::io::sink(),
        written: 0,
    };
    postcard::to_io(value, &mut counted).context("unable to encode")?;
    Ok(counted.written)
}

/// Decodes a value this process wrote itself — a journal record, a cache
/// entry — where the bytes are as trustworthy as the disk they came from.
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    postcard::from_bytes(bytes).context("unable to decode")
}

/// Decodes a value that arrived from somewhere else, refusing one
/// longer than [`CEILING`] before it is looked at.
pub fn decode_capped<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    anyhow::ensure!(
        bytes.len() <= CEILING,
        "a message of {} bytes is past the {CEILING} this build decodes",
        bytes.len(),
    );
    decode(bytes)
}

/// A writer that counts what passes through it.
struct Counting<W> {
    inner: W,
    written: u64,
}

impl<W: std::io::Write> std::io::Write for Counting<W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let wrote = self.inner.write(bytes)?;
        self.written += wrote as u64;
        Ok(wrote)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fixed expectation rather than a round trip: a round trip
    /// passes for any self-consistent format, including one that would
    /// make every machine's state unreadable. These bytes are what the
    /// checkpoint format promises, so a change to them has to be a
    /// change to that number too.
    #[test]
    fn the_format_is_the_one_on_disk() {
        #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
        struct Record {
            generation: u64,
            name: String,
            executable: bool,
        }
        let record = Record {
            generation: 1,
            name: "a".to_owned(),
            executable: true,
        };
        let bytes = encode(&record).expect("encodes");
        assert_eq!(
            bytes,
            vec![
                1,    // the generation, a varint
                1,    // the string's length, the same
                b'a', // and its one byte
                1,    // the bool
            ],
        );
        assert_eq!(decode::<Record>(&bytes).expect("decodes"), record);
    }

    /// What `size_of` says is what `encode` writes. They are separate
    /// paths — a sink and a buffer — and a journal that reserved one
    /// length and wrote another would tear every record after it.
    #[test]
    fn a_measured_size_is_the_size_written() {
        let value = (
            "a string of some length".to_owned(),
            70_000u64,
            vec![1u8; 9],
        );
        assert_eq!(
            size_of(&value).expect("sized"),
            encode(&value).expect("encodes").len() as u64
        );
    }

    /// A message past the ceiling is refused before it is decoded.
    #[test]
    fn a_message_past_the_ceiling_is_refused() {
        let under = encode(&vec![0u8; 8]).expect("encodes");
        assert!(decode_capped::<Vec<u8>>(&under).is_ok());

        let over = vec![0u8; CEILING + 1];
        let refused = decode_capped::<Vec<u8>>(&over);
        assert!(refused.is_err(), "{refused:?}");
    }
}
