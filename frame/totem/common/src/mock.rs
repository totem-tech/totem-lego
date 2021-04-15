#![cfg(any(test, feature = "mock"))]

use super::traits::accounting::Posting;
use frame_support::dispatch::DispatchResultWithPostInfo;
use sp_std::vec::Vec;

impl<AccountId, Hash, BlockNumber, CoinAmount> Posting<AccountId, Hash, BlockNumber, CoinAmount> for () {
    type Account = ();
    type PostingIndex = u128;
    type LedgerBalance = i128;

    fn handle_multiposting_amounts(
        fwd: Vec<(AccountId, Self::Account, Self::LedgerBalance, bool, Hash, BlockNumber, BlockNumber)>,
        rev: Vec<(AccountId, Self::Account, Self::LedgerBalance, bool, Hash, BlockNumber, BlockNumber)>,
        trk: Vec<(AccountId, Self::Account, Self::LedgerBalance, bool, Hash, BlockNumber, BlockNumber)>,
    ) -> DispatchResultWithPostInfo {
        unimplemented!("Used as a mock, shouldn't be called")
    }

    fn account_for_fees(f: CoinAmount, p: AccountId) -> DispatchResultWithPostInfo {
        unimplemented!("Used as a mock, shouldn't be called")
    }

    fn get_escrow_account() -> AccountId {
        unimplemented!("Used as a mock, shouldn't be called")
    }

    fn get_pseudo_random_hash(s: AccountId, r: AccountId) -> Hash {
        unimplemented!("Used as a mock, shouldn't be called")
    }
}
