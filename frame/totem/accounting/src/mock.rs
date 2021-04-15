#![cfg(any(test, feature = "mock"))]

use sp_runtime::traits::Convert;

pub struct Conversions;

impl Convert<u128, i128> for Conversions {
    fn convert(u: u128) -> i128 {
        u as i128
    }
}

impl Convert<i128, i128> for Conversions {
    fn convert(u: i128) -> i128 {
        u
    }
}

impl Convert<u64, i128> for Conversions {
    fn convert(u: u64) -> i128 {
        u as i128
    }
}
