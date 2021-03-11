use node_primitives::Hash;
use sp_runtime::traits::Convert;
use sp_std::vec::Vec;

// Totem implemented for converting between Accounting Balances and Internal Balances
pub struct ConversionHandler;

// Basic type conversion
impl ConversionHandler {
    fn signed_to_unsigned(x: i128) -> u128 {
        x.abs() as u128
    }
}

// Takes the AccountBalance and converts for use with BalanceOf<T>
impl Convert<i128, u128> for ConversionHandler {
    fn convert(x: i128) -> u128 {
        Self::signed_to_unsigned(x) as u128
    }
}

// Takes BalanceOf<T> and converts for use with AccountBalance type
impl Convert<u128, i128> for ConversionHandler {
    fn convert(x: u128) -> i128 {
        x as i128
    }
}

// Takes integer u64 and converts for use with AccountOf<T> type
impl Convert<u64, u64> for ConversionHandler {
    fn convert(x: u64) -> u64 {
        x
    }
}

// Takes integer u64 and converts for use with BlockNumber
impl Convert<u32, u32> for ConversionHandler {
    fn convert(x: u32) -> u32 {
        x
    }
}

// Takes integer u64 or AccountOf<T> and converts for use with BalanceOf<T> type
impl Convert<u64, u128> for ConversionHandler {
    fn convert(x: u64) -> u128 {
        x as u128
    }
}

// Takes integer i128 and inverts for use mainly with AccountBalanceOf<T> type
impl Convert<i128, i128> for ConversionHandler {
    fn convert(x: i128) -> i128 {
        x
    }
}

// Used for extracting a user's balance into an integer for calculations
impl Convert<u128, u128> for ConversionHandler {
    fn convert(x: u128) -> u128 {
        x
    }
}

// Used to convert to associated type UnLocked<T>
impl Convert<bool, bool> for ConversionHandler {
    fn convert(x: bool) -> bool {
        x
    }
}

// Takes Vec<u8> encoded hash and converts for as a LockIdentifier type
impl Convert<Vec<u8>, [u8; 8]> for ConversionHandler {
    fn convert(x: Vec<u8>) -> [u8; 8] {
        let mut y: [u8; 8] = [0; 8];
        for z in 0..8 {
            y[z] = x[z].into();
        }
        return y;
    }
}

// Used to convert hashes
impl Convert<Hash, Hash> for ConversionHandler {
    fn convert(x: Hash) -> Hash {
        x
    }
}

impl Convert<bool, pallet_prefunding::LockStatus> for ConversionHandler {
    fn convert(x: bool) -> pallet_prefunding::LockStatus {
        if x {
            pallet_prefunding::LockStatus::Locked
        } else {
            pallet_prefunding::LockStatus::Unlocked
        }
    }
}
