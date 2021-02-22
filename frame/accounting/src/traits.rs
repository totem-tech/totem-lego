use frame_support::pallet_prelude::*;
use sp_std::prelude::*;

/// Main Totem trait.
pub trait Posting<AccountId, Hash, BlockNumber, CoinAmount> {
    type Account: Member + Copy + Eq;
    type PostingIndex: Member + Copy + Into<u128> + Encode + Decode + Eq;
    type LedgerBalance: Member + Copy + Into<i128> + Encode + Decode + Eq;

    fn handle_multiposting_amounts(
        fwd: Vec<(AccountId, Self::Account, Self::LedgerBalance, bool, Hash, BlockNumber, BlockNumber)>,
        rev: Vec<(AccountId, Self::Account, Self::LedgerBalance, bool, Hash, BlockNumber, BlockNumber)>,
        trk: Vec<(AccountId, Self::Account, Self::LedgerBalance, bool, Hash, BlockNumber, BlockNumber)>,
    ) -> DispatchResultWithPostInfo;

    fn account_for_fees(f: CoinAmount, p: AccountId) -> DispatchResultWithPostInfo;

    fn get_escrow_account() -> AccountId;

    fn get_pseudo_random_hash(s: AccountId, r: AccountId) -> Hash;
}
