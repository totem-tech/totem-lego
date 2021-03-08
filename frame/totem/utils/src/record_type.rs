use frame_support::{dispatch::EncodeLike, pallet_prelude::*};

#[repr(u8)]
#[derive(Decode, Encode, Debug, Clone, Copy, PartialEq)]
pub enum RecordType {
    Teams,
    Timekeeping,
    Orders,
}

impl EncodeLike<RecordType> for u8 {}
