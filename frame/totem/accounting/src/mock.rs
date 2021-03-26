#![cfg(test)]

use sp_runtime::traits::Convert;

struct Conversions;

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
