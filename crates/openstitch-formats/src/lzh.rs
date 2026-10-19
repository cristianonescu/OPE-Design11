//! The LZ77 + Huffman compression used by the Husqvarna Viking / Pfaff HUS and VIP stream
//! sections. It is the static-Huffman block coding of LHA's `-lh5-` family (public descriptions:
//! the LHA format notes and Haruhiko Okumura's public-domain `ar002`), as read by pyembroidery's
//! `EmbCompress.expand` (MIT), which is the oracle for this module.
//!
//! Bit stream, most significant bit first. Each block:
//! - 16 bits: number of tokens in the block;
//! - the "length tree" (19 symbols): 5-bit count, then each code length as a 3-bit value,
//!   where 7 continues in unary (`1` adds one, `0` stops); after the third length, 2 bits skip
//!   up to three zero lengths. A count of 0 is followed by a 5-bit symbol used for every code;
//! - the character tree (510 symbols, literals 0–255 and match lengths 3–256 as 256–509):
//!   9-bit count, then lengths coded with the length tree, where symbol 0 is one zero length,
//!   1 is 3 + 4 bits zero lengths, 2 is 20 + 9 bits zero lengths and n ≥ 3 is length n − 2;
//! - the distance tree: 5-bit count, then lengths as for the length tree, without the skip;
//! - the tokens. A match's distance code `p` is followed by `p − 1` extra bits; the distance
//!   back is `(1 << (p − 1)) + extra + 1` (or 1 when `p` is 0).
//!
//! Codes are canonical: shorter codes first, ties by symbol. The writer keeps distances within
//! 8 KiB and code lengths within 16 bits, as `-lh5-` does.

use crate::{FormatError, Result};

const NT: usize = 19;
const NC: usize = 510;
const NP: usize = 14;
const MAX_BITS: u32 = 16;
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 256;
const WINDOW: usize = 1 << (NP - 1);
const BLOCK_TOKENS: usize = 0xFFFF;
const END_SYMBOL: u16 = 510;
const UNASSIGNED: u16 = u16::MAX;

// MARK: Decompression

struct BitReader<'a> {
    data: &'a [u8],
    position: usize,
}

impl BitReader<'_> {
    fn total_bits(&self) -> usize {
        self.data.len() * 8
    }

    /// `count` (≤ 16) bits at the current position; bits past the end read as zero.
    fn peek(&self, count: u32) -> u32 {
        let mut value = 0_u32;
        for i in 0..count as usize {
            let bit = self.position + i;
            let byte = self.data.get(bit / 8).copied().unwrap_or(0);
            value = value << 1 | u32::from(byte >> (7 - bit % 8) & 1);
        }
        value
    }

    fn pop(&mut self, count: u32) -> u32 {
        let v = self.peek(count);
        self.position += count as usize;
        v
    }

    fn variable_length(&mut self) -> u32 {
        let mut m = self.pop(3);
        if m == 7 {
            for _ in 0..13 {
                if self.pop(1) == 1 {
                    m += 1;
                } else {
                    break;
                }
            }
        }
        m
    }
}

/// A decoding table: either a single symbol that consumes no bits, or a lookup indexed by the
/// next `width` bits.
enum Table {
    Single(u16),
    Lookup {
        width: u32,
        symbols: Vec<u16>,
        lengths: Vec<u8>,
    },
}

impl Table {
    fn build(lengths: &[u8]) -> Result<Self> {
        let width = u32::from(lengths.iter().copied().max().unwrap_or(0));
        if width == 0 || width > MAX_BITS {
            return Err(FormatError::ValueOutOfRange(format!(
                "compressed stream: Huffman code length {width}"
            )));
        }
        let size = 1_usize << width;
        let mut symbols = Vec::with_capacity(size);
        for bits in 1..=width {
            for (symbol, &l) in lengths.iter().enumerate() {
                if u32::from(l) == bits {
                    let span = size >> bits;
                    if symbols.len() + span > size {
                        return Err(FormatError::ValueOutOfRange(
                            "compressed stream: over-subscribed Huffman code".into(),
                        ));
                    }
                    symbols.resize(symbols.len() + span, symbol as u16);
                }
            }
        }
        // An incomplete code is accepted (libembroidery's literal-only streams declare one);
        // only reaching an unassigned code is an error.
        symbols.resize(size, UNASSIGNED);
        Ok(Self::Lookup {
            width,
            symbols,
            lengths: lengths.to_vec(),
        })
    }

    fn decode(&self, bits: &mut BitReader) -> Result<u16> {
        match self {
            Self::Single(v) => Ok(*v),
            Self::Lookup {
                width,
                symbols,
                lengths,
            } => {
                let s = symbols[(bits.peek(16) >> (16 - width)) as usize];
                if s == UNASSIGNED {
                    return Err(FormatError::ValueOutOfRange(
                        "compressed stream: unassigned Huffman code".into(),
                    ));
                }
                bits.position += usize::from(lengths[usize::from(s)]);
                Ok(s)
            }
        }
    }
}

fn read_length_table(bits: &mut BitReader, count_bits: u32, skip_at_three: bool) -> Result<Table> {
    let count = bits.pop(count_bits) as usize;
    if count == 0 {
        return Ok(Table::Single(bits.pop(count_bits) as u16));
    }
    let mut lengths = vec![0_u8; count];
    let mut index = 0;
    while index < count {
        if skip_at_three && index == 3 {
            index += bits.pop(2) as usize;
            if index >= count {
                break;
            }
        }
        lengths[index] = bits.variable_length() as u8;
        index += 1;
    }
    Table::build(&lengths)
}

fn read_character_table(bits: &mut BitReader, length_table: &Table) -> Result<Table> {
    let count = bits.pop(9) as usize;
    if count == 0 {
        return Ok(Table::Single(bits.pop(9) as u16));
    }
    let mut lengths = vec![0_u8; count];
    let mut index = 0;
    while index < count {
        match length_table.decode(bits)? {
            0 => index += 1,
            1 => index += 3 + bits.pop(4) as usize,
            2 => index += 20 + bits.pop(9) as usize,
            c => {
                lengths[index] = u8::try_from(c - 2).unwrap_or(u8::MAX);
                index += 1;
            }
        }
    }
    Table::build(&lengths)
}

/// Expands one compressed stream to at most `limit` bytes (the record count from the file
/// header). Malformed codes return an error; a stream that ends early returns what it decoded.
pub fn expand(data: &[u8], limit: usize) -> Result<Vec<u8>> {
    let mut bits = BitReader { data, position: 0 };
    let mut out: Vec<u8> = Vec::with_capacity(limit.min(1 << 20));
    let mut remaining_in_block = 0_u32;
    let mut tables: Option<(Table, Table)> = None;
    while bits.total_bits() > bits.position && out.len() < limit {
        if remaining_in_block == 0 {
            remaining_in_block = bits.pop(16);
            let length_table = read_length_table(&mut bits, 5, true)?;
            let characters = read_character_table(&mut bits, &length_table)?;
            let distances = read_length_table(&mut bits, 5, false)?;
            tables = Some((characters, distances));
            if remaining_in_block == 0 {
                continue;
            }
        }
        remaining_in_block -= 1;
        let Some((characters, distances)) = &tables else {
            break;
        };
        let symbol = characters.decode(&mut bits)?;
        if symbol <= 0xFF {
            out.push(symbol as u8);
            continue;
        }
        if symbol >= END_SYMBOL {
            break;
        }
        let length = usize::from(symbol) - 253;
        let p = u32::from(distances.decode(&mut bits)?);
        if p > 24 {
            return Err(FormatError::ValueOutOfRange(format!(
                "compressed stream: distance code {p}"
            )));
        }
        let back = if p == 0 {
            1
        } else {
            (1_usize << (p - 1)) + bits.pop(p - 1) as usize + 1
        };
        let Some(start) = out.len().checked_sub(back) else {
            return Err(FormatError::ValueOutOfRange(format!(
                "compressed stream: distance {back} before the start"
            )));
        };
        for i in 0..length.min(limit - out.len()) {
            let b = out[start + i];
            out.push(b);
        }
    }
    Ok(out)
}

// MARK: Compression

#[derive(Default)]
struct BitWriter {
    bytes: Vec<u8>,
    used: u32,
}

impl BitWriter {
    fn put(&mut self, count: u32, value: u32) {
        for i in (0..count).rev() {
            if self.used % 8 == 0 {
                self.bytes.push(0);
            }
            let bit = (value >> i & 1) as u8;
            let last = self.bytes.len() - 1;
            self.bytes[last] |= bit << (7 - self.used % 8);
            self.used += 1;
        }
    }

    fn put_variable_length(&mut self, length: u8) {
        if length < 7 {
            self.put(3, u32::from(length));
        } else {
            self.put(3, 7);
            let ones = u32::from(length) - 7;
            self.put(ones, (1 << ones) - 1);
            if ones < 13 {
                self.put(1, 0);
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Token {
    Literal(u8),
    Match { length: usize, distance: usize },
}

/// Greedy LZ77 with hash chains over 3-byte prefixes.
fn tokenize(data: &[u8]) -> Vec<Token> {
    const HASH: usize = 1 << 13;
    const CHAIN: usize = 128;
    let hash = |i: usize| {
        (usize::from(data[i]) << 10 ^ usize::from(data[i + 1]) << 5 ^ usize::from(data[i + 2]))
            & (HASH - 1)
    };
    let mut head = vec![usize::MAX; HASH];
    let mut previous = vec![usize::MAX; data.len()];
    let insert = |i: usize, head: &mut [usize], previous: &mut [usize]| {
        if i + MIN_MATCH <= data.len() {
            let h = hash(i);
            previous[i] = head[h];
            head[h] = i;
        }
    };
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let (mut best_length, mut best_distance) = (0, 0);
        if i + MIN_MATCH <= data.len() {
            let mut candidate = head[hash(i)];
            let limit = (data.len() - i).min(MAX_MATCH);
            for _ in 0..CHAIN {
                if candidate == usize::MAX || i - candidate > WINDOW {
                    break;
                }
                let length = (0..limit)
                    .take_while(|&k| data[candidate + k] == data[i + k])
                    .count();
                if length > best_length {
                    (best_length, best_distance) = (length, i - candidate);
                    if length == limit {
                        break;
                    }
                }
                candidate = previous[candidate];
            }
        }
        if best_length >= MIN_MATCH {
            tokens.push(Token::Match {
                length: best_length,
                distance: best_distance,
            });
            for k in i..i + best_length {
                insert(k, &mut head, &mut previous);
            }
            i += best_length;
        } else {
            tokens.push(Token::Literal(data[i]));
            insert(i, &mut head, &mut previous);
            i += 1;
        }
    }
    tokens
}

/// Distance code and its extra bits for a distance of `distance` (≥ 1).
fn distance_code(distance: usize) -> (usize, u32, u32) {
    let d = (distance - 1) as u32;
    if d == 0 {
        return (0, 0, 0);
    }
    let bits = 32 - d.leading_zeros();
    (bits as usize, bits - 1, d - (1 << (bits - 1)))
}

/// Huffman code lengths limited to `MAX_BITS`, by the JPEG (ITU T.81 Annex K.3) adjustment.
fn code_lengths(frequencies: &[u32]) -> Vec<u8> {
    let mut lengths = vec![0_u8; frequencies.len()];
    let used: Vec<usize> = (0..frequencies.len())
        .filter(|&s| frequencies[s] > 0)
        .collect();
    if used.len() < 2 {
        return lengths;
    }
    // Plain Huffman over (weight, node); leaves are 0..n, internal nodes follow.
    let mut parent = vec![usize::MAX; used.len() * 2 - 1];
    let mut heap: std::collections::BinaryHeap<std::cmp::Reverse<(u64, usize)>> = used
        .iter()
        .enumerate()
        .map(|(node, &s)| std::cmp::Reverse((u64::from(frequencies[s]), node)))
        .collect();
    let mut next = used.len();
    while heap.len() > 1 {
        let std::cmp::Reverse((wa, a)) = heap.pop().expect("two nodes");
        let std::cmp::Reverse((wb, b)) = heap.pop().expect("two nodes");
        parent[a] = next;
        parent[b] = next;
        heap.push(std::cmp::Reverse((wa + wb, next)));
        next += 1;
    }
    let depth = |mut node: usize| {
        let mut d = 0_usize;
        while parent[node] != usize::MAX {
            node = parent[node];
            d += 1;
        }
        d
    };
    let mut count = vec![0_u32; used.len() + 1];
    for leaf in 0..used.len() {
        count[depth(leaf)] += 1;
    }
    let max = MAX_BITS as usize;
    for i in (max + 1..count.len()).rev() {
        while count[i] > 0 {
            let mut j = i - 2;
            while count[j] == 0 {
                j -= 1;
            }
            count[i] -= 2;
            count[i - 1] += 1;
            count[j + 1] += 2;
            count[j] -= 1;
        }
    }
    // Shortest codes to the most frequent symbols.
    let mut by_frequency = used;
    by_frequency.sort_by(|&a, &b| frequencies[b].cmp(&frequencies[a]).then(a.cmp(&b)));
    let mut symbols = by_frequency.into_iter();
    for (length, &n) in count.iter().enumerate().take(max + 1) {
        for _ in 0..n {
            if let Some(s) = symbols.next() {
                lengths[s] = length as u8;
            }
        }
    }
    lengths
}

/// Canonical codes for `lengths`, matching the decoder's table order.
fn canonical_codes(lengths: &[u8]) -> Vec<u32> {
    let mut codes = vec![0_u32; lengths.len()];
    let mut code = 0_u32;
    for bits in 1..=MAX_BITS {
        for (s, &l) in lengths.iter().enumerate() {
            if u32::from(l) == bits {
                codes[s] = code;
                code += 1;
            }
        }
        code <<= 1;
    }
    codes
}

/// A tree to emit: one symbol written as a count of 0, or lengths and codes.
struct Tree {
    lengths: Vec<u8>,
    codes: Vec<u32>,
    single: Option<usize>,
}

impl Tree {
    fn new(frequencies: &[u32]) -> Self {
        let used: Vec<usize> = (0..frequencies.len())
            .filter(|&s| frequencies[s] > 0)
            .collect();
        let single = (used.len() < 2).then(|| used.first().copied().unwrap_or(0));
        let lengths = code_lengths(frequencies);
        let codes = canonical_codes(&lengths);
        Self {
            lengths,
            codes,
            single,
        }
    }

    /// Number of lengths to transmit (up to the last non-zero one).
    fn count(&self) -> usize {
        self.lengths
            .iter()
            .rposition(|&l| l > 0)
            .map_or(0, |i| i + 1)
    }

    fn put(&self, w: &mut BitWriter, symbol: usize) {
        if self.single.is_none() {
            w.put(u32::from(self.lengths[symbol]), self.codes[symbol]);
        }
    }

    /// Writes a tree whose lengths use the 3-bit variable-length form.
    fn write_lengths(&self, w: &mut BitWriter, count_bits: u32, skip_at_three: bool) {
        if let Some(s) = self.single {
            w.put(count_bits, 0);
            w.put(count_bits, s as u32);
            return;
        }
        let n = self.count();
        w.put(count_bits, n as u32);
        let mut i = 0;
        while i < n {
            if skip_at_three && i == 3 {
                let zeros = self.lengths[3..n]
                    .iter()
                    .take(3)
                    .take_while(|&&l| l == 0)
                    .count();
                w.put(2, zeros as u32);
                i += zeros;
            }
            w.put_variable_length(self.lengths[i]);
            i += 1;
        }
    }
}

/// The character-tree lengths as length-tree symbols plus extra bits (zero runs coded 0/1/2).
fn character_length_symbols(lengths: &[u8], n: usize) -> Vec<(usize, u32, u32)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        if lengths[i] != 0 {
            out.push((usize::from(lengths[i]) + 2, 0, 0));
            i += 1;
            continue;
        }
        let run = lengths[i..n].iter().take_while(|&&l| l == 0).count();
        i += run;
        match run {
            1 | 2 => out.extend(std::iter::repeat_n((0, 0, 0), run)),
            3..=18 => out.push((1, 4, run as u32 - 3)),
            19 => {
                out.push((0, 0, 0));
                out.push((1, 4, 15));
            }
            _ => out.push((2, 9, run as u32 - 20)),
        }
    }
    out
}

fn write_block(w: &mut BitWriter, tokens: &[Token]) {
    let mut char_freq = vec![0_u32; NC];
    let mut dist_freq = vec![0_u32; NP];
    for t in tokens {
        match *t {
            Token::Literal(b) => char_freq[usize::from(b)] += 1,
            Token::Match { length, distance } => {
                char_freq[length + 253] += 1;
                dist_freq[distance_code(distance).0] += 1;
            }
        }
    }
    let characters = Tree::new(&char_freq);
    let distances = Tree::new(&dist_freq);

    w.put(16, tokens.len() as u32);
    if let Some(s) = characters.single {
        Tree::new(&[0; NT]).write_lengths(w, 5, true);
        w.put(9, 0);
        w.put(9, s as u32);
    } else {
        let n = characters.count();
        let symbols = character_length_symbols(&characters.lengths, n);
        let mut length_freq = vec![0_u32; NT];
        for &(s, _, _) in &symbols {
            length_freq[s] += 1;
        }
        let length_tree = Tree::new(&length_freq);
        length_tree.write_lengths(w, 5, true);
        w.put(9, n as u32);
        for (s, extra_bits, extra) in symbols {
            length_tree.put(w, s);
            w.put(extra_bits, extra);
        }
    }
    distances.write_lengths(w, 5, false);

    for t in tokens {
        match *t {
            Token::Literal(b) => characters.put(w, usize::from(b)),
            Token::Match { length, distance } => {
                characters.put(w, length + 253);
                let (code, extra_bits, extra) = distance_code(distance);
                distances.put(w, code);
                w.put(extra_bits, extra);
            }
        }
    }
}

/// Compresses `data` into one or more blocks of at most 65 535 tokens.
pub fn compress(data: &[u8]) -> Vec<u8> {
    let tokens = tokenize(data);
    let mut w = BitWriter::default();
    for block in tokens.chunks(BLOCK_TOKENS) {
        write_block(&mut w, block);
    }
    w.bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(data: &[u8]) {
        let packed = compress(data);
        assert_eq!(expand(&packed, data.len()).expect("expand"), data);
    }

    #[test]
    fn round_trips_edge_cases() {
        round_trip(&[]);
        round_trip(&[7]);
        round_trip(&[0x80; 1000]);
        round_trip(&[1, 2]);
        round_trip(b"abcabcabcabcabcabcabd");
        let mut noise = Vec::new();
        let mut s = 0x1234_5678_u32;
        for _ in 0..200_000 {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            noise.push((s % 7) as u8 * 31);
        }
        round_trip(&noise);
    }

    #[test]
    fn all_literals_and_lengths_round_trip() {
        let mut data: Vec<u8> = (0..=255).collect();
        for n in 3..300 {
            data.extend(std::iter::repeat_n((n % 256) as u8, n));
        }
        round_trip(&data);
    }

    #[test]
    fn compresses_repetitive_data() {
        let data = [0x80_u8; 10_000];
        assert!(compress(&data).len() < 200);
    }

    #[test]
    fn deep_trees_are_length_limited() {
        // Fibonacci frequencies force a Huffman depth well over 16.
        let mut freq = vec![0_u32; NC];
        let (mut a, mut b) = (1_u32, 1_u32);
        for f in freq.iter_mut().take(30) {
            *f = a;
            (a, b) = (b, a.saturating_add(b));
        }
        let lengths = code_lengths(&freq);
        assert!(lengths.iter().all(|&l| u32::from(l) <= MAX_BITS));
        let kraft: f64 = lengths
            .iter()
            .filter(|&&l| l > 0)
            .map(|&l| 2f64.powi(-i32::from(l)))
            .sum();
        assert!((kraft - 1.0).abs() < 1e-9);
        Table::build(&lengths).expect("complete code");
    }

    #[test]
    fn rejects_bad_streams() {
        assert!(expand(&[0xFF; 64], 1000).is_err());
        // libembroidery's literal-only form (incomplete distance code), with a big-endian
        // block size: every byte is a literal.
        let mut stored = vec![0x00, 0x05, 0x02, 0xA0, 0x01, 0xFE];
        stored.extend_from_slice(b"hello");
        assert_eq!(expand(&stored, 5).expect("stored"), b"hello");
    }
}
