
//! LEB128 variable length integers.
//!
//! There are signed and unsigned versions, and versions with heap-allocated
//! or borrowed backing storage. The latter allow for use in zero-allocation
//! libraries.
//!
//! We support encoding and decoding to all Rust integer types and to arrays of
//! bytes.

#![feature(core_intrinsics)]
use std::mem;

/// Signed LEB128 integer.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct ILeb128(Vec<u8>);

/// Unsigned LEB128 integer.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct ULeb128(Vec<u8>);

/// Signed LEB128 integer, backed by a reference.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct ILeb128Ref<'a>(&'a [u8]);

/// Unsigned LEB128 integer, backed by a reference.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct ULeb128Ref<'a>(&'a [u8]);

// TODO PartialEq to compare Ref with non-Ref versions

macro_rules! dispatch {
    ($name: ident, $typ: ty) => {
        pub fn $name(&self) -> $typ {
            self.as_ref().$name()
        }
    }
}

impl ILeb128 {
    pub fn from_bytes(bytes: &[u8]) -> ILeb128 {
        ILeb128Ref::from_bytes(bytes).to_owned()
    }

    pub fn all_from_bytes(bytes: &[u8]) -> Vec<ILeb128> {
        ILeb128Ref::all_from_bytes(bytes).into_iter().map(|i| i.to_owned()).collect()
    }

    pub fn byte_count(self) -> usize {
        self.0.len()
    }

    pub fn as_ref(&self) -> ILeb128Ref {
        ILeb128Ref(&self.0)
    }

    dispatch!(expect_i8, i8);
    dispatch!(expect_i16, i16);
    dispatch!(expect_i32, i32);
    dispatch!(expect_i64, i64);
    dispatch!(expect_i128, [u8; 16]);
    dispatch!(decode_bytes, Vec<u8>);
}

impl ULeb128 {
    pub fn from_bytes(bytes: &[u8]) -> ULeb128 {
        ULeb128Ref::from_bytes(bytes).to_owned()
    }

    pub fn all_from_bytes(bytes: &[u8]) -> Vec<ULeb128> {
        ULeb128Ref::all_from_bytes(bytes).into_iter().map(|i| i.to_owned()).collect()
    }

    pub fn byte_count(self) -> usize {
        self.0.len()
    }

    pub fn as_ref(&self) -> ULeb128Ref {
        ULeb128Ref(&self.0)
    }

    dispatch!(expect_u8, u8);
    dispatch!(expect_u16, u16);
    dispatch!(expect_u32, u32);
    dispatch!(expect_u64, u64);
    dispatch!(expect_u128, [u8; 16]);
    dispatch!(decode_bytes, Vec<u8>);
}

macro_rules! decode_signed {
    ($name: ident, $t: ty) => {
        pub fn $name(self) -> $t {
            let mut result = 0;
            let mut shift = 0;
            let bit_count = mem::size_of::<$t>() * 8;
            for &byte in self.0 {
                result |= (byte & 0b0111_1111) as $t << shift;
                shift += 7;
                if byte & 0b1000_0000 == 0 {
                    break;
                }
            }

            let last_byte = self.0[self.0.len() - 1];
            unsafe {
                // I.e., if the last byte is positive
                let size = if (last_byte & 0b0100_0000) == 0 {
                    shift + 1 - std::intrinsics::ctlz(last_byte) as usize
                } else {
                    // Count the leading ones up to the first significant one.
                    shift + 2 - std::intrinsics::ctlz(!(last_byte | 0b1000_0000)) as usize
                };
                assert!(size <= mem::size_of::<$t>() * 8,
                        "overflow, expected {} byte(s)", mem::size_of::<$t>());
            }

            // Sign extend if necessary.
            if shift < bit_count && (last_byte & 0b0100_0000) != 0 {
                result |= ((1 << shift) as $t).wrapping_neg();
            }
            result
        }        
    }
}

macro_rules! leb_ref_impl {
    ($t: ident, $owned_t: ident) => {
        /// Read a single valid LEB128 number from bytes.
        /// Panics if there is not a valid LEB128 number in bytes.
        pub fn from_bytes(bytes: &'a [u8]) -> $t<'a> {
            let mut count = 0;
            for byte in bytes {
                count += 1;
                if byte & 0b1000_0000 == 0 {
                    return $t(&bytes[0..count]);
                }
            }
            panic!("from_bytes on invalid input");
        }

        /// Read all of bytes into a Vec of LEB128 numbers. Panics if there
        /// are trailing bytes which are not part of a valid LEB128 number.
        pub fn all_from_bytes(bytes: &'a [u8]) -> Vec<$t<'a>> {
            let mut result = vec![];
            let mut start = 0;
            let mut end = 0;
            for byte in bytes {
                end += 1;
                if byte & 0b1000_0000 == 0 {
                    result.push($t(&bytes[start..end]));
                    start = end;
                }
            }
            assert!(start == end, "all_from_bytes on invalid input");
            
            result
        }

        pub fn byte_count(self) -> usize {
            self.0.len()
        }

        pub fn to_owned(self) -> $owned_t {
            $owned_t(self.0.to_owned())
        }
    }
}

impl<'a> ILeb128Ref<'a> {
    leb_ref_impl!(ILeb128Ref, ILeb128);

    decode_signed!(expect_i8, i8);
    decode_signed!(expect_i16, i16);
    decode_signed!(expect_i32, i32);
    decode_signed!(expect_i64, i64);

    // Returns the bytes in little-endian order, since Rust doesn't have a u128
    // type.
    pub fn expect_i128(self) -> [u8; 16] {
        unimplemented!();
    }

    // Prefer expect_* since they don't need to do any heap allocation.
    pub fn decode_bytes(self) -> Vec<u8> {
        unimplemented!();
    }
}

macro_rules! decode_unsigned {
    ($name: ident, $t: ty) => {
        pub fn $name(self) -> $t {
            let mut result = 0;
            let mut shift = 0;
            for &byte in self.0 {
                result |= (byte & 0b0111_1111) as $t << shift;
                shift += 7;
                if byte & 0b1000_0000 == 0 {
                    break;
                }
            }

            unsafe {
                let size = shift + 1 - std::intrinsics::ctlz(self.0[self.0.len() - 1]) as usize;
                assert!(size <= mem::size_of::<$t>() * 8,
                        "overflow, expected {} byte(s)", mem::size_of::<$t>());
            }
            result
        }        
    }
}

impl<'a> ULeb128Ref<'a> {
    leb_ref_impl!(ULeb128Ref, ULeb128);

    decode_unsigned!(expect_u8, u8);
    decode_unsigned!(expect_u16, u16);
    decode_unsigned!(expect_u32, u32);
    decode_unsigned!(expect_u64, u64);

    // Returns the bytes in little-endian order, since Rust doesn't have a u128
    // type.
    pub fn expect_u128(self) -> [u8; 16] {
        unimplemented!();
    }

    // Prefer expect_* since they don't need to do any heap allocation.
    pub fn decode_bytes(self) -> Vec<u8> {
        unimplemented!();
    }
}


pub trait ToILeb128: Sized {
    fn encode(self) -> ILeb128;
}

pub trait ToULeb128: Sized {
    fn encode(self) -> ULeb128;
}

macro_rules! impl_encode_signed {
    ($t: ident) => {
        impl ToILeb128 for $t {
            fn encode(mut self) -> ILeb128 {
                const SIGN_BIT: u8 = 0b0100_0000;
                let mut result = vec![];
                let mut more = true;
                loop {
                    let mut byte = self as u8 & 0b0111_1111;
                    self >>= 7;
                    if (self == 0 && byte & SIGN_BIT == 0) ||
                       (self == -1 && byte & SIGN_BIT != 0) {
                        more = false;
                    } else {
                        byte |= 0b1000_0000;
                    }
                    result.push(byte);

                    if !more {
                        return ILeb128(result);
                    }
                }
            }
        }
    }
}

macro_rules! impl_encode_unsigned {
    ($t: ident) => {
        impl ToULeb128 for $t {
            fn encode(mut self) -> ULeb128 {
                let mut result = vec![];
                loop {
                    let mut byte = self as u8 & 0b0111_1111;
                    self >>= 7;
                    if self != 0 {
                        byte |= 0b1000_0000;
                    }
                    result.push(byte);

                    if self == 0 {
                        return ULeb128(result);
                    }
                }
            }
        }
    }
}

impl_encode_signed!(i8);
impl_encode_signed!(i16);
impl_encode_signed!(i32);
impl_encode_signed!(i64);
impl_encode_unsigned!(u8);
impl_encode_unsigned!(u16);
impl_encode_unsigned!(u32);
impl_encode_unsigned!(u64);

impl<'a> ToILeb128 for &'a [u8] {
    fn encode(self) -> ILeb128 {
        unimplemented!();
    }
}

impl<'a> ToULeb128 for &'a [u8] {
    fn encode(self) -> ULeb128 {
        unimplemented!();
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_unsigned_encode() {
        assert!((0u8).encode() == ULeb128::from_bytes(&[0]));
        assert!((42u8).encode() == ULeb128::from_bytes(&[42]));
        assert!((127u8).encode() == ULeb128::from_bytes(&[127]));
        assert!((128u8).encode() == ULeb128::from_bytes(&[128, 1]));
        assert!((255u8).encode() == ULeb128::from_bytes(&[255, 1]));

        assert!((0u16).encode() == ULeb128::from_bytes(&[0]));
        assert!((42u16).encode() == ULeb128::from_bytes(&[42]));
        assert!((127u16).encode() == ULeb128::from_bytes(&[127]));
        assert!((128u16).encode() == ULeb128::from_bytes(&[128, 1]));
        assert!((0xffffu16).encode() == ULeb128::from_bytes(&[255, 255, 0b11]));

        assert!((0u32).encode() == ULeb128::from_bytes(&[0]));
        assert!((42u32).encode() == ULeb128::from_bytes(&[42]));
        assert!((127u32).encode() == ULeb128::from_bytes(&[127]));
        assert!((128u32).encode() == ULeb128::from_bytes(&[128, 1]));
        assert!((624485u32).encode() == ULeb128::from_bytes(&[0xE5, 0x8E, 0x26]));

        assert!((0u64).encode() == ULeb128::from_bytes(&[0]));
        assert!((42u64).encode() == ULeb128::from_bytes(&[42]));
        assert!((127u64).encode() == ULeb128::from_bytes(&[127]));
        assert!((128u64).encode() == ULeb128::from_bytes(&[128, 1]));
        assert!((624485u64).encode() == ULeb128::from_bytes(&[0xE5, 0x8E, 0x26]));
    }

    #[test]
    fn test_signed_encode() {
        assert!((   0i8).encode() == ILeb128::from_bytes(&[0]));
        assert!((   2i8).encode() == ILeb128::from_bytes(&[2]));
        assert!((  -2i8).encode() == ILeb128::from_bytes(&[0x7e]));
        assert!(( 127i8).encode() == ILeb128::from_bytes(&[0xff, 0]));
        assert!((-127i8).encode() == ILeb128::from_bytes(&[0x81, 0x7f]));
        assert!((-128i8).encode() == ILeb128::from_bytes(&[0x80, 0x7f]));

        assert!((   0i16).encode() == ILeb128::from_bytes(&[0]));
        assert!((   2i16).encode() == ILeb128::from_bytes(&[2]));
        assert!((  -2i16).encode() == ILeb128::from_bytes(&[0x7e]));
        assert!(( 127i16).encode() == ILeb128::from_bytes(&[0xff, 0]));
        assert!((-127i16).encode() == ILeb128::from_bytes(&[0x81, 0x7f]));
        assert!(( 128i16).encode() == ILeb128::from_bytes(&[0x80, 1]));
        assert!((-128i16).encode() == ILeb128::from_bytes(&[0x80, 0x7f]));
        assert!(( 129i16).encode() == ILeb128::from_bytes(&[0x81, 1]));
        assert!((-129i16).encode() == ILeb128::from_bytes(&[0xff, 0x7e]));

        assert!((   0i32).encode() == ILeb128::from_bytes(&[0]));
        assert!((   2i32).encode() == ILeb128::from_bytes(&[2]));
        assert!((  -2i32).encode() == ILeb128::from_bytes(&[0x7e]));
        assert!(( 127i32).encode() == ILeb128::from_bytes(&[0xff, 0]));
        assert!((-127i32).encode() == ILeb128::from_bytes(&[0x81, 0x7f]));
        assert!(( 128i32).encode() == ILeb128::from_bytes(&[0x80, 1]));
        assert!((-128i32).encode() == ILeb128::from_bytes(&[0x80, 0x7f]));
        assert!(( 129i32).encode() == ILeb128::from_bytes(&[0x81, 1]));
        assert!((-129i32).encode() == ILeb128::from_bytes(&[0xff, 0x7e]));

        assert!((   0i64).encode() == ILeb128::from_bytes(&[0]));
        assert!((   2i64).encode() == ILeb128::from_bytes(&[2]));
        assert!((  -2i64).encode() == ILeb128::from_bytes(&[0x7e]));
        assert!(( 127i64).encode() == ILeb128::from_bytes(&[0xff, 0]));
        assert!((-127i64).encode() == ILeb128::from_bytes(&[0x81, 0x7f]));
        assert!(( 128i64).encode() == ILeb128::from_bytes(&[0x80, 1]));
        assert!((-128i64).encode() == ILeb128::from_bytes(&[0x80, 0x7f]));
        assert!(( 129i64).encode() == ILeb128::from_bytes(&[0x81, 1]));
        assert!((-129i64).encode() == ILeb128::from_bytes(&[0xff, 0x7e]));
    }

    #[test]
    fn test_unsigned_decode() {
        assert!(ULeb128::from_bytes(&[0]).expect_u8() == 0);
        assert!(ULeb128::from_bytes(&[42]).expect_u8() == 42);
        assert!(ULeb128::from_bytes(&[127]).expect_u8() == 127);
        assert!(ULeb128::from_bytes(&[128, 1]).expect_u8() == 128);
        assert!(ULeb128::from_bytes(&[255, 1]).expect_u8() == 255);

        assert!(ULeb128::from_bytes(&[0]).expect_u16() == 0);
        assert!(ULeb128::from_bytes(&[42]).expect_u16() == 42);
        assert!(ULeb128::from_bytes(&[127]).expect_u16() == 127);
        assert!(ULeb128::from_bytes(&[128, 1]).expect_u16() == 128);
        assert!(ULeb128::from_bytes(&[255, 255, 3]).expect_u16() == 0xffff);

        assert!(ULeb128::from_bytes(&[0]).expect_u32() == 0);
        assert!(ULeb128::from_bytes(&[42]).expect_u32() == 42);
        assert!(ULeb128::from_bytes(&[127]).expect_u32() == 127);
        assert!(ULeb128::from_bytes(&[128, 1]).expect_u32() == 128);
        assert!(ULeb128::from_bytes(&[0xE5, 0x8E, 0x26]).expect_u32() == 624485);
        assert!(ULeb128::from_bytes(&[255, 255, 255, 255, 0b1111]).expect_u32() == 0xffff_ffff);

        assert!(ULeb128::from_bytes(&[0]).expect_u64() == 0);
        assert!(ULeb128::from_bytes(&[42]).expect_u64() == 42);
        assert!(ULeb128::from_bytes(&[127]).expect_u64() == 127);
        assert!(ULeb128::from_bytes(&[128, 1]).expect_u64() == 128);
        assert!(ULeb128::from_bytes(&[0xE5, 0x8E, 0x26]).expect_u64() == 624485);
        assert!(ULeb128::from_bytes(&[255, 255, 255, 255, 255, 255, 255, 255, 255, 1]).expect_u64() == 0xffff_ffff_ffff_ffff);
    }

    #[test]
    fn test_signed_decode() {
        println!("5");
        assert!(ILeb128::from_bytes(&[0]).expect_i8() == 0);
        println!("6");
        assert!(ILeb128::from_bytes(&[0]).expect_i8() == 0);
        println!("7");
        assert!(ILeb128::from_bytes(&[2]).expect_i8() == 2);
        println!("8");
        assert!(ILeb128::from_bytes(&[0x7e]).expect_i8() == -2);
        println!("9");
        assert!(ILeb128::from_bytes(&[0xff, 0]).expect_i8() == 127);
        println!("2");
        assert!(ILeb128::from_bytes(&[0x81, 0x7f]).expect_i8() == -127);
        println!("3");
        assert!(ILeb128::from_bytes(&[0x80, 0x7f]).expect_i8() == -128);
        println!("4");

        assert!(ILeb128::from_bytes(&[0]).expect_i16() == 0);
        assert!(ILeb128::from_bytes(&[0]).expect_i16() == 0);
        assert!(ILeb128::from_bytes(&[2]).expect_i16() == 2);
        assert!(ILeb128::from_bytes(&[0x7e]).expect_i16() == -2);
        assert!(ILeb128::from_bytes(&[0xff, 0]).expect_i16() == 127);
        assert!(ILeb128::from_bytes(&[0x81, 0x7f]).expect_i16() == -127);
        assert!(ILeb128::from_bytes(&[0x80, 1]).expect_i16() == 128);
        assert!(ILeb128::from_bytes(&[0x80, 0x7f]).expect_i16() == -128);
        assert!(ILeb128::from_bytes(&[0x81, 1]).expect_i16() == 129);
        assert!(ILeb128::from_bytes(&[0xff, 0x7e]).expect_i16() == -129);

        assert!(ILeb128::from_bytes(&[0]).expect_i32() == 0);
        assert!(ILeb128::from_bytes(&[0]).expect_i32() == 0);
        assert!(ILeb128::from_bytes(&[2]).expect_i32() == 2);
        assert!(ILeb128::from_bytes(&[0x7e]).expect_i32() == -2);
        assert!(ILeb128::from_bytes(&[0xff, 0]).expect_i32() == 127);
        assert!(ILeb128::from_bytes(&[0x81, 0x7f]).expect_i32() == -127);
        assert!(ILeb128::from_bytes(&[0x80, 1]).expect_i32() == 128);
        assert!(ILeb128::from_bytes(&[0x80, 0x7f]).expect_i32() == -128);
        assert!(ILeb128::from_bytes(&[0x81, 1]).expect_i32() == 129);
        assert!(ILeb128::from_bytes(&[0xff, 0x7e]).expect_i32() == -129);

        assert!(ILeb128::from_bytes(&[0]).expect_i64() == 0);
        assert!(ILeb128::from_bytes(&[0]).expect_i64() == 0);
        assert!(ILeb128::from_bytes(&[2]).expect_i64() == 2);
        assert!(ILeb128::from_bytes(&[0x7e]).expect_i64() == -2);
        assert!(ILeb128::from_bytes(&[0xff, 0]).expect_i64() == 127);
        assert!(ILeb128::from_bytes(&[0x81, 0x7f]).expect_i64() == -127);
        assert!(ILeb128::from_bytes(&[0x80, 1]).expect_i64() == 128);
        assert!(ILeb128::from_bytes(&[0x80, 0x7f]).expect_i64() == -128);
        assert!(ILeb128::from_bytes(&[0x81, 1]).expect_i64() == 129);
        assert!(ILeb128::from_bytes(&[0xff, 0x7e]).expect_i64() == -129);
    }

    #[test]
    #[should_panic]
    fn test_decode_overflow_u8() {
        ULeb128::from_bytes(&[128, 2]).expect_u8();
    }
    #[test]
    #[should_panic]
    fn test_decode_overflow_u16() {
        ULeb128::from_bytes(&[128, 128, 4]).expect_u16();
    }
    #[test]
    #[should_panic]
    fn test_decode_overflow_u32() {
        ULeb128::from_bytes(&[128, 128, 128, 128, 16]).expect_u32();
    }
    #[test]
    #[should_panic]
    fn test_decode_overflow_u64() {
        ULeb128::from_bytes(&[128, 128, 128, 128, 128, 128, 128, 128, 128, 2]).expect_u64();
    }
    #[test]
    #[should_panic]
    fn test_decode_overflow_i8() {
        ILeb128::from_bytes(&[128, 2]).expect_i8();
    }
    #[test]
    #[should_panic]
    fn test_decode_overflow_i16() {
        ILeb128::from_bytes(&[128, 128, 4]).expect_i16();
    }
    #[test]
    #[should_panic]
    fn test_decode_overflow_i32() {
        ILeb128::from_bytes(&[128, 128, 128, 128, 16]).expect_i32();
    }
    #[test]
    #[should_panic]
    fn test_decode_overflow_i64() {
        ILeb128::from_bytes(&[128, 128, 128, 128, 128, 128, 128, 128, 128, 2]).expect_i64();
    }

    #[test]
    fn test_byte_count() {
        assert!(ILeb128::from_bytes(&[2]).byte_count() == 1);
        assert!(ILeb128::from_bytes(&[128, 128, 128, 2]).byte_count() == 4);
        assert!(ILeb128::from_bytes(&[128, 128, 128, 128, 128, 128, 128, 128, 128, 2]).byte_count() == 10);

        assert!(ULeb128::from_bytes(&[2]).byte_count() == 1);
        assert!(ULeb128::from_bytes(&[128, 128, 128, 2]).byte_count() == 4);
        assert!(ULeb128::from_bytes(&[128, 128, 128, 128, 128, 128, 128, 128, 128, 2]).byte_count() == 10);
    }

    // TODO test invalid from_bytes
    // TODO test all_from_bytes (including invalid bytes)
}
