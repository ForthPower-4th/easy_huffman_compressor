use std::collections::HashMap;
use std::io::{Read, Write};
use std::io;
use crate::huffman::HuffmanTree;
use crate::huffman::TreeNode;
//this field is used for limit the maximum depth of huffman_tree in decoder
const MAX_HUFFMAN_DEPTH: u8 = 16;

pub struct Encoder< W: Write> {
    text: String,
    writer : BitWriter<W>,
    huffman_code: HashMap<char, BinaryCode>,
    preorder_leaf_char : Vec<char>,
    frequency_map: HashMap<char, usize>,
    binarized_tree: Vec<bool>,
    total_byte_num : usize,
}
impl<W> Encoder<W> where W: Write {
    pub fn open(encoded: String, writer : W) -> Option<Encoder<W>>{
        if encoded.is_empty(){
            return None;
        }

        let frequency_map = Self::generate_frequency_map(&encoded);
        // if the input str is empty, then the frequency map will be empty, leading to function parse_huffman_tree returning None
        let huffman_tree = crate::huffman::parse_huffman_tree(&frequency_map).unwrap();
        let (huffman_code, preorder_leaf_char, binarized_tree)= Self::generate_binarize_tree_and_build_dict(huffman_tree);
        // the pre-order tree is written bit by bit and padded to a byte boundary before the dictionary
        let tree_bytes = binarized_tree.len().div_ceil(8);
        // the dictionary is written as UTF-8 bytes, so multibyte characters take more than one byte
        let dictionary_bytes: usize = preorder_leaf_char.iter().map(|c| c.len_utf8()).sum();
        // the body is written bit by bit and padded only once, at the end
        let body_bits: usize = preorder_leaf_char.iter().map(|c|{
            (frequency_map.get(c).unwrap()) * (huffman_code.get(c).unwrap().len as usize)
        }).sum();
        let total_byte_num:usize = 3 + 4 + 8// magic_number byte + the num of dictionary character (u32) + 3.the num of total characters (8 bytes)
            + tree_bytes
            + dictionary_bytes
            + body_bits.div_ceil(8);
        Some(Encoder{
            text:encoded,
            writer: BitWriter::new(writer),
            frequency_map,
            huffman_code,
            preorder_leaf_char,
            binarized_tree,
            total_byte_num
        })
    }
    pub fn get_total_byte_size(&self) -> usize{
        self.total_byte_num
    }
    pub fn encode(mut self) -> Result<(), io::Error>{
        // write the file head
        // structure:
        // 1.the U8 of "HFM" (3 bytes)
        // 2.the num of dictionary character (4 byte, be_byte)
        // 3.the num of total characters (8 bytes, be_byte)
        // 3.pre-order traverse of Huffman tree, 1:leaf, 0:branch
        // 4.dictionary characters in the order of pre-order traverse, 0:left,1:right

        self.writer.write_bytes(b"HFM")?;
        //step 1. encode Huffman tree
        //step 2. write the num of dictionary characters (4 bytes, be_byte)
        self.writer.write_bytes(&(self.frequency_map.len() as u32).to_be_bytes())?;
        //step 3.write the num of total characters
        self.writer.write_bytes(&(self.text.chars().count() as u64).to_be_bytes())?;

        //step4. write binarized_tree
        for &bit in self.binarized_tree.iter(){
            self.writer.write_bit(bit)?;
        }
        //step5. write dictionary characters
        self.writer.write_bytes(self.preorder_leaf_char.iter().collect::<String>().as_bytes())?;
        //write the file body
        for each in self.text.chars(){
            let Some(&BinaryCode{binary, len}) = self.huffman_code.get(&each) else{
                unreachable!("the character is not in the huffman_tree");
            };
            self.writer.write_u32_bits(binary, len)?;
        }
        self.writer.flush()?;
        Ok(())
    }
    fn generate_frequency_map(text:&str) -> HashMap<char, usize>{
        let mut count: HashMap<char, usize> = HashMap::new();
        for each in text.chars(){
            *count.entry(each).or_insert(0) += 1;
        }
        count
    }
    fn generate_binarize_tree_and_build_dict(huffman_tree: HuffmanTree) -> (HashMap<char, BinaryCode>, Vec<char>, Vec<bool>){
        //preorder-traverse to get binary encode, and store it into tree_bin(Vec<bool>)
        let mut stack: Vec<(&TreeNode,BinaryCode)> = vec![(&*huffman_tree.root, BinaryCode{binary: 0, len: 0})];
        //huffman_code: the dictionary from a character to corresponding Huffman code
        //0 &TreeNode: reference of the node,
        //1 BinaryCode: the path of the node,
        //BinaryCode.binary u32: the path of the current node, i.e. the binary of the current character,
        //BinaryCode.len: num of significant bits (== depth of current node)
        let mut huffman_code: HashMap<char, BinaryCode> =  HashMap::new();
        //leaf_chars store the character according to the order of traverse in order to store it into head of file
        let mut preorder_binarized_tree = Vec::new();
        let mut preorder_leaf_chars : Vec<char> = Vec::new();
        while let Some((node, BinaryCode{binary , len})) = stack.pop(){ // binary == path , len == depth
            match node{
                TreeNode::Leaf{character, ..} => {
                    huffman_code.insert(*character, BinaryCode{binary, len});
                    preorder_leaf_chars.push(*character);
                    preorder_binarized_tree.push(true); // leaf  -> 1
                }
                TreeNode::Branch{left_children, right_children, ..} => {
                    let path_right = (binary << 1) | 1; // right -> 1
                    stack.push((&*right_children, BinaryCode{binary:path_right, len:len + 1}));
                    let path_left = binary << 1 ; // left -> 0
                    stack.push((&*left_children, BinaryCode{binary:path_left, len:len + 1}));
                    preorder_binarized_tree.push(false); // branch -> 0
                }
            }
        }
        (huffman_code,preorder_leaf_chars,preorder_binarized_tree)
    }
}

pub struct Decoder<R : Read>{
    reader : BitReader<R>,
}
impl<R : Read> Decoder<R> {
    pub fn open(reader: R) -> Self {
        Self {
            reader: BitReader::new(reader),
        }
    }
    pub fn decode(mut self) -> io::Result<String> {
        self.reader.expect(b"HFM")?;
        let char_num = {
            let mut raw_char_num: [u8; 4] = [0u8; 4];
            self.reader.read_exact(&mut raw_char_num)?;
            u32::from_be_bytes(raw_char_num)
        };

        let total = {
            let mut raw_char_num: [u8; 8] = [0u8; 8];
            self.reader.read_exact(&mut raw_char_num)?;
            u64::from_be_bytes(raw_char_num)
        };
        let codes: Vec<BinaryCode> = self.read_tree_code()?; // get the binary codes of leaves in preorder of the huffman tree
        if codes.len() != char_num as usize{
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "preorder tree has {} leaves, expected {}",
                    codes.len(),
                    char_num
                )));
        }

        let mut chars = Vec::with_capacity(char_num as usize);
        for _ in 0..char_num {
            chars.push(self.reader.read_character()?);
        }
        let table = Self::build_tree_table(chars, codes, MAX_HUFFMAN_DEPTH); // Builds a flat (single-level) lookup table for Huffman decoding
        let mut result = String::new();
        for _ in 0..total{
            let (scan, avail) = self.reader.peek_32()?;

            let (character, take_up_bits) = Self::read_huffman_code_from_table(&table, scan)?;
            result.push(character);
            if avail < take_up_bits{
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Unexpected end of file"));
            }
            else{
                self.reader.consume(take_up_bits as u32)?;
            }
        }
        Ok(result)
    }
    fn read_tree_code(&mut self) -> Result<Vec<BinaryCode>, io::Error>{
        let mut binary_code:u32 = 0;
        let mut path : Vec<bool> = Vec::new();
        let mut codes = Vec::new();
        'outer :loop {
            while !self.reader.read_bit()? {
                path.push(false);
                binary_code <<= 1;
            }
            codes.push(BinaryCode{binary: binary_code, len: path.len() as u8});
            loop{
                match path.last(){
                    Some( false) => {
                        *path.last_mut().unwrap() = true; // we can refer to path.last(), then the path.last_mut() must exist.
                        binary_code |= 1;
                        break;
                    }
                    Some( true) =>{
                        path.pop();
                        binary_code >>= 1;
                    }
                    None =>{
                        break 'outer;
                    }
                }
            }
        }
        Ok(codes)
    }
    fn build_tree_table(chars:Vec<char>, codes:Vec<BinaryCode>, table_depth:u8) -> Box<[DecodeEntry]>{
        // Look-up Table Structure
        // Aims to achieve nearly O(1) lookup complexity
        // 1. Short codes: expanded and stored directly as DecodeEntry
        // 2. Long  codes: choose between two strategies ro balance memory and speed
        //  -Subtable: mirrors the supertable structure; memory-intensive nut O(1)
        //  -Linear table: scan-based lookup; memory-effective but degrades to O(n)
        let mut vec_table = Vec::with_capacity(1usize << table_depth);
        let mut i = 0;
        let n = codes.len(); // chars.len() == codes.len()
        while i < n{
            let len  = codes[i].len ;
            let character = chars[i];
            if len <= table_depth {
                // short codes are expanded to fill all `2^(MAX - len)` table slots sharing that prefix.
                for _ in 0..1usize << (table_depth - len) {
                    let entry = DecodeEntry::Direct{ character, len};
                    // `chars` and `codes` are provided in Huffman-tree preorder, so the code
                    // intervals are disjoint and strictly increasing; pushing in this order
                    // writes `table` sequentially from low to high indices.
                    vec_table.push(entry);
                }
                i += 1;
            }
            else{
                let mut deep_code_map_char = Vec::<char>::new();
                let mut deep_code_map_binary = Vec::<BinaryCode>::new();
                // codes[i] (type:BinaryCode).binary : LSB-aligned
                let prefix = codes[i].binary >> (len - table_depth);
                // cnt : number of deep codes sharing same prefix.
                // determine whether to use subtable or linear table storage.
                let mut deep_code_num = 0;
                let mut max_suffix_len = 0;
                while i < n{
                    // 1. Stop if we encounter a short code (start of a new subtree).
                    if codes[i].len <= table_depth {
                        break;
                    }
                    let suffix_len = codes[i].len - table_depth;
                    let code = codes[i].binary;
                    // 2. Stop if the prefix changes.
                    if (code >> suffix_len) != prefix {
                        break;
                    }
                    // BinaryCode struct requires LSB-aligned
                    let mask = u32::MAX >> (32 - suffix_len);
                    let suffix = code & mask;
                    let character = chars[i];
                    deep_code_map_char.push(character);
                    deep_code_map_binary.push(BinaryCode{binary: suffix, len:suffix_len});
                    deep_code_num += 1;
                    if suffix_len > max_suffix_len{
                        max_suffix_len = suffix_len;
                    }
                    i += 1
                }
                // Determine the best data structure based on density and memory span.
                // `span` = 1 << max_suffix_len. If max_suffix_len > 10 (span > 1024, i.e., 8KB), it's too large.
                let stored = if deep_code_num <= 8  || max_suffix_len > 10{
                    // Sparse leaves or too large span: use a linear table (SubVec) to save memory and cache lines.
                    DecodeEntry::SubVec(deep_code_map_char.into_iter().zip(deep_code_map_binary).collect())
                }
                else{
                    // Dense leaves and manageable span: use a subtable to achieve O(1) lookup time.
                    DecodeEntry::SubTable(Self::build_tree_table(deep_code_map_char, deep_code_map_binary, max_suffix_len))
                };
                vec_table.push(stored);
            }
        }
        // Guarantee the table is fully populated.
        debug_assert_eq!(vec_table.len(), 1usize << table_depth);
        vec_table.into_boxed_slice()
    }
    pub fn read_huffman_code_from_table(table: &Box<[DecodeEntry]>, scan: u32) -> Result<(char, u8), io::Error>{
        //return : the char and the len of then DecodeEntry
        // the table size must be the power of 2
        let supertable_depth = table.len().trailing_zeros() as u8;
        let idx = (scan >> (32 - supertable_depth)) as usize;
        let next_scan = scan << supertable_depth;
        match &table[idx] {
            DecodeEntry::Direct { character, len } => {Ok((*character, *len ))}
            DecodeEntry::SubVec(vec) => {

                for(character, binary_code) in vec{
                    let suffix = next_scan >> (32 - binary_code.len);
                    if suffix == binary_code.binary{
                        return Ok((*character, binary_code.len + supertable_depth));
                    }
                }
                unreachable!("The suffix must be in the linear table");
            }
            DecodeEntry::SubTable (subtable) => {
                let next_scan = scan << supertable_depth;
                let (character, subtable_depth) = Self::read_huffman_code_from_table(subtable, next_scan)?;
                Ok((character, subtable_depth + supertable_depth))
            }
        }

    }
}

struct BitReader<R: Read> {
    reader: R,
    buffer: u64,
    bit_pos: usize, // Read position in current byte. 8 means no byte is buffered.
    valid_byte:usize,
    is_eof: bool,
}

impl<R: Read> BitReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            buffer: 0,
            bit_pos: 64,
            valid_byte:0,
            is_eof:false
        }
    }

    // Reads one bit (MSB first). Loads a new byte if the current one is exhausted.
    pub fn read_bit(&mut self) -> io::Result<bool> {
        if self.valid_byte * 8 <= self.bit_pos{
            self.refill()?;
            if self.is_eof{
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Unexpected EOF of the Bit Reader"));
            }
        }

        let bit = (self.buffer >> (63 - self.bit_pos)) & 1 == 1;
        self.bit_pos += 1;
        Ok(bit)
    }
    // Refill the buffer
    pub fn refill(&mut self) -> Result<(), io::Error>{
        fn read_helper_func<R : Read>(reader: &mut R, bytes: &mut [u8]) -> Result<usize, io::Error>{
            // this helper function is aimed to tackle with the EOF/Interrupted circumstances
            let mut n_read = 0;
            while n_read < bytes.len() {
                match reader.read(&mut bytes[n_read..]) {
                    Ok(0) => break,                                    // 真 EOF
                    Ok(n) => n_read += n,                               // 短读，继续
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                }
            }
            Ok(n_read)
        }
        if self.is_eof { return Ok(()); }
        if self.bit_pos == 64{
            let mut bytes = [0u8;8];
            let input_valid_byte_num = read_helper_func(&mut self.reader, &mut bytes)?;
            if input_valid_byte_num == 0{
                self.is_eof = true;
            }
            self.buffer = u64::from_be_bytes(bytes);
            self.bit_pos = 0;
            // the buffer is now cleared, then the valid_byte num == input byte num
            self.valid_byte = input_valid_byte_num;
            return Ok(())
        }
        let bytes_discarded_num = self.bit_pos / 8;
        self.buffer <<= bytes_discarded_num * 8;
        self.bit_pos = self.bit_pos % 8;

        let mut mask =  [0u8;8];
        let input_valid_byte_num = read_helper_func(&mut self.reader, &mut mask[8 - bytes_discarded_num..])?;
        if input_valid_byte_num <  bytes_discarded_num{
            self.is_eof = true;
        }
        self.buffer |= u64::from_be_bytes(mask) << (64 - self.valid_byte * 8 );
        self.valid_byte = self.valid_byte - bytes_discarded_num + input_valid_byte_num;
        Ok(())
    }

    // Discards remaining bits in the current byte.
    pub fn align_to_byte(&mut self){
        debug_assert!(self.bit_pos <= 64);
        self.bit_pos = (self.bit_pos + 7)/ 8 * 8;
    }

    // Reads exactly buf.len() bytes (aligns to byte boundary first).
    pub fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        debug_assert!(self.bit_pos <= 64);
        if buf.is_empty(){
            return Ok(());
        }
        self.align_to_byte();
        //warrant there will be min(buf.len, 8) bytes in buffer
        let available = (self.valid_byte * 8).saturating_sub(self.bit_pos)  / 8;
        if available < buf.len().min(8) && !self.is_eof {
            self.refill()?;
        }

        let available = (self.valid_byte * 8).saturating_sub(self.bit_pos)  / 8;
        if available == 0{
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Unexpected EOF"));
        }
        //copy from buffer
        let take_from_buffer = available.min(buf.len());
        let start = self.bit_pos / 8;
        buf[..take_from_buffer]
            .copy_from_slice(&self.buffer.to_be_bytes()[start..start + take_from_buffer]);
        self.bit_pos += take_from_buffer * 8;
        if take_from_buffer == buf.len() {
            return Ok(());
        }
        self.reader.read_exact(&mut buf[take_from_buffer..])?;
        self.bit_pos = self.valid_byte * 8;
        Ok(())
    }

    // Reads exactly one UTF-8 character.
    pub fn read_character(&mut self) -> io::Result<char> {
        self.align_to_byte();

        let mut bytes = [0u8; 4];
        self.read_exact(&mut bytes[..1])?;
        let lead = bytes[0];

        // Determine UTF-8 sequence length from the leading byte.
        let need = match lead {
            0x00..=0x7F => 0,
            0x80..=0xBF => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Continuation byte cannot be a lead byte",
                ));
            }
            0xC0..=0xC1 => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Overlong encoding",
                ));
            }
            0xC2..=0xDF => 1,
            0xE0..=0xEF => 2,
            0xF0..=0xF4 => 3,
            0xF5..=0xFF => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Exceeds U+10FFFF or invalid pattern",
                ));
            }
        };

        if need > 0 {
            self.read_exact(&mut bytes[1..=need])?;
        }

        let s = std::str::from_utf8(&bytes[..=need])
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

        s.chars()
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Empty character"))
    }

    // Validates that the next bytes match the expected magic bytes.
    pub fn expect(&mut self, expected: &[u8]) -> io::Result<()> {
        let mut buf = vec![0u8; expected.len()];
        self.read_exact(&mut buf)?;

        if buf != expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Mismatch: expected {:?}, found {:?}", expected, buf),
            ));
        }
        Ok(())
    }
    pub fn peek_32(&mut self) -> io::Result<(u32, u8)> {
        if self.valid_byte * 8 - self.bit_pos < 32 && !self.is_eof {
            self.refill()?;
        }
        let avail = (self.valid_byte * 8)
            .saturating_sub(self.bit_pos)
            .min(32) as u8;
        let shifted = if self.bit_pos >= 64 { 0 }
        else { self.buffer << self.bit_pos };
        Ok(((shifted >> 32) as u32, avail))
    }
    pub fn consume(&mut self, n: u32) -> io::Result<()> {
        if self.bit_pos + n as usize > self.valid_byte * 8 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "consume past end"));
        }
        self.bit_pos += n as usize;
        Ok(())
    }
}
struct BitWriter<W : Write>{
    writer : W,
    pending_bits : u8, // The pending bits are stored LSB-aligned
    remain_bit_num : u8,
}
impl<W : Write> BitWriter<W>{
    pub fn new(writer: W) -> Self{
        Self{
            writer,
            pending_bits : 0,
            remain_bit_num : 0,
        }
    }
    pub fn write_bit(&mut self, bit: bool) -> io::Result<()>{
        self.pending_bits  = (self.pending_bits << 1) | (bit as u8);
        self.remain_bit_num += 1;
        if self.remain_bit_num == 8{
            self.writer.write_all(&[self.pending_bits])?;
            self.pending_bits = 0;
            self.remain_bit_num = 0;
        }
        Ok(())
    }
    pub fn write_bytes(&mut self, input:&[u8]) -> io::Result<()>{
        if self.remain_bit_num != 0{
            self.writer.write_all(&[self.pending_bits << (8 - self.remain_bit_num)])?;
            self.remain_bit_num = 0;
            self.pending_bits = 0;
        }
        self.writer.write_all(input)?;
        Ok(())
    }
    pub fn write_u32_bits(&mut self, binary:u32, num:u8) -> io::Result<()>{
        for i in (0..num).rev(){
            let bit = (binary >> i) & 1 == 1;
            self.write_bit(bit)?;
        }
        Ok(())
    }
    pub fn flush(&mut self) -> io::Result<()>{
        if self.remain_bit_num != 0 {
            self.writer.write_all(&[self.pending_bits << (8 - self.remain_bit_num)])?;
            self.pending_bits = 0;
            self.remain_bit_num = 0;
        }
        self.writer.flush()?;
        Ok(())
    }
}
#[derive(Debug, Clone)]
pub struct BinaryCode{
    binary : u32, // LSB - aligned
    len : u8
}
#[derive(Debug)]
pub enum DecodeEntry{
    Direct{
        character : char,
        len :u8,
    },
    SubVec(Vec<(char,BinaryCode)>),
    SubTable(Box<[DecodeEntry]>),
}