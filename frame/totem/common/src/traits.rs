//                              Næ§@@@ÑÉ©
//                        æ@@@@@@@@@@@@@@@@@@
//                    Ñ@@@@?.?@@@@@@@@@@@@@@@@@@@N
//                 ¶@@@@@?^%@@.=@@@@@@@@@@@@@@@@@@@@
//               N@@@@@@@?^@@@»^@@@@@@@@@@@@@@@@@@@@@@
//               @@@@@@@@?^@@@».............?@@@@@@@@@É
//              Ñ@@@@@@@@?^@@@@@@@@@@@@@@@@@@'?@@@@@@@@Ñ
//              @@@@@@@@@?^@@@»..............»@@@@@@@@@@
//              @@@@@@@@@?^@@@»^@@@@@@@@@@@@@@@@@@@@@@@@
//              @@@@@@@@@?^ë@@&.@@@@@@@@@@@@@@@@@@@@@@@@
//               @@@@@@@@?^´@@@o.%@@@@@@@@@@@@@@@@@@@@©
//                @@@@@@@?.´@@@@@ë.........*.±@@@@@@@æ
//                 @@@@@@@@?´.I@@@@@@@@@@@@@@.&@@@@@N
//                  N@@@@@@@@@@ë.*=????????=?@@@@@Ñ
//                    @@@@@@@@@@@@@@@@@@@@@@@@@@@¶
//                        É@@@@@@@@@@@@@@@@Ñ¶
//                             Næ§@@@ÑÉ©

// Copyright 2020 Chris D'Costa
// This file is part of Totem Live Accounting.
// Author Chris D'Costa email: chris.dcosta@totemaccounting.com

// Totem is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Totem is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Totem.  If not, see <http://www.gnu.org/licenses/>.

use frame_support::{dispatch::EncodeLike, pallet_prelude::*};
use sp_runtime::traits::Member;
use sp_std::prelude::*;

pub mod accounting {
    use super::*;

    #[repr(u8)]
    #[derive(Decode, Encode, Clone, Copy)]
    pub enum Indicator {
        Debit = 0,
        Credit = 1,
    }
    impl EncodeLike<Indicator> for bool {}
    impl Indicator {
        pub fn reverse(self) -> Self {
            match self {
                Self::Debit => Self::Credit,
                Self::Credit => Self::Debit,
            }
        }
    }

    #[derive(Clone)]
    pub struct Record<AccountId, Hash, BlockNumber, Account, LedgerBalance> {
        pub primary_party: AccountId,
        pub counterparty: AccountId,
        pub ledger_account: Account,
        pub amount: LedgerBalance,
        pub debit_credit: Indicator,
        pub reference_hash: Hash,
        pub changed_on_blocknumber: BlockNumber,
        pub applicable_period_blocknumber: BlockNumber,
    }

    impl<AccountId, Hash, BlockNumber, Account, LedgerBalance>
        Record<AccountId, Hash, BlockNumber, Account, LedgerBalance>
    {
        pub fn new(
            primary_party: AccountId,
            counterparty: AccountId,
            ledger_account: Account,
            amount: LedgerBalance,
            debit_credit: Indicator,
            reference_hash: Hash,
            changed_on_blocknumber: BlockNumber,
            applicable_period_blocknumber: BlockNumber,
        ) -> Self {
            Record {
                primary_party,
                counterparty,
                ledger_account,
                amount,
                debit_credit,
                reference_hash,
                changed_on_blocknumber,
                applicable_period_blocknumber,
            }
        }
    }

    /// Main Totem accounting trait.
    pub trait Posting<AccountId, Hash, BlockNumber, CoinAmount> {
        type Account: Member + Copy + Eq;
        type PostingIndex: Member + Copy + Into<u128> + Encode + Decode + Eq;
        type LedgerBalance: Member + Copy + Into<i128> + Encode + Decode + Eq;

        fn handle_multiposting_amounts(
            keys: Vec<Record<AccountId, Hash, BlockNumber, Self::Account, Self::LedgerBalance>>,
        ) -> DispatchResultWithPostInfo;

        fn account_for_fees(f: CoinAmount, p: AccountId) -> DispatchResultWithPostInfo;

        fn get_escrow_account() -> AccountId;

        fn get_pseudo_random_hash(s: AccountId, r: AccountId) -> Hash;
    }
}

pub mod bonsai {
    use super::*;

    pub trait Storing<Hash> {
        fn claim_data(r: Hash, d: Hash) -> DispatchResultWithPostInfo;

        fn start_tx(u: Hash) -> DispatchResultWithPostInfo;

        fn end_tx(u: Hash) -> DispatchResultWithPostInfo;
    }
}

pub mod prefunding {
    use super::*;

    pub trait Encumbrance<AccountId, Hash, BlockNumber> {
        type LockStatus: Member + Copy;

        fn prefunding_for(
            who: AccountId,
            recipient: AccountId,
            amount: u128,
            deadline: BlockNumber,
            ref_hash: Hash,
            uid: Hash,
        ) -> DispatchResultWithPostInfo;

        fn send_simple_invoice(o: AccountId, p: AccountId, n: i128, h: Hash, uid: Hash) -> DispatchResultWithPostInfo;

        fn settle_prefunded_invoice(o: AccountId, h: Hash, uid: Hash) -> DispatchResultWithPostInfo;

        fn set_release_state(o: AccountId, o_lock: Self::LockStatus, h: Hash, uid: Hash) -> DispatchResultWithPostInfo;

        fn unlock_funds_for_owner(o: AccountId, h: Hash, uid: Hash) -> DispatchResultWithPostInfo;

        fn check_ref_owner(o: AccountId, h: Hash) -> bool;

        fn check_ref_beneficiary(o: AccountId, h: Hash) -> bool;
    }
}

pub mod orders {
    pub trait Validating<AccountId, Hash> {
        fn is_order_party(o: AccountId, r: Hash) -> bool;
    }
}

pub mod teams {
    pub trait Validating<AccountId, Hash> {
        fn is_project_owner(o: AccountId, h: Hash) -> bool;

        fn is_owner_and_project_valid(o: AccountId, h: Hash) -> bool;

        fn is_project_valid(h: Hash) -> bool;
    }
}

pub mod timekeeping {
    pub trait Validating<AccountId, Hash> {
        fn is_time_record_owner(o: AccountId, h: Hash) -> bool;

        fn validate_and_archive(o: AccountId, h: Hash, a: bool) -> bool;
    }
}
