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

//! Provides a decentralised authority for data storage.
//!
//! In Totem we require an off-chain searchable database that may end up containing billions of records.
//! IPFS is not a solution as the type of data to be stored may be queried, editied, and each time IPFS cannot overwrite or update existing datasets.
//! Additionally IPFS may drop files that are not considered current, used or needed, which is not ideal for static records like invoices.
//!
//! We wanted a solution where permission for storing an editing data should not be dependent on third-party authentication and access
//! was global, recoverable and self-sovereign.
//!
//! Bonsai is a simple protocol, for allowing independent databases to come to a consensus on content.
//! It works by assuming that the data to be stored must be previously authenticated by its owner on-chain.
//!
//! # How it works
//!
//! Firstly, a reference to the record is created either on-chain or offchain by an account which immediately becomes its owner.
//! The reference is a hash (H256) with sufficient entropy to be unique per the record.
//! A transaction is sent to the blockchain at some point associating the reference to an address for the first time.
//! The reference is considered to be the key to some other data which is not suitable for onchain storage, but will be stored in an offchain database.
//! The offchain database will only accept new or changing records, provided that it can
//! a) find the reference hash onchain, and
//! b) an associated data-hash which it also finds on chain with a hash of the incoming data.
//! The data may be plaintext or encrypted, neither matters as long as the hash of this data matches onchain data-hash.
//! As the on-chain transaction validates the signature, the off-chain database does not need to authenticate the client that communicates
//! the insertion or change request as it has already been "pre-authorised" by the blockchain runtime.
//! Totem believes there is a fee market for storage in this model.
//!
//! # Process
//!
//! A third party database receives a request to store some data. The Database queries the blockchain to find out:
//!
//! 1. Does the reference hash exist on chain and of it does, then collect the associated data-hash also stored onchain;
//! 2. Upon confirmation the reference hash exists, hashing the received data and compare the data-hash to the one found on chain. If it does not match, then do nothing
//! (effectively rejecting the attempt to store the data), and if it does match then store the data using the reference hash as the key;
//! 3. In the event that an reference hash already exists, the data-hash obtained from the blockchain is always king. Provided it matches, overwrite exiting data.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{fail, pallet_prelude::*};
use frame_system::pallet_prelude::*;

use sp_primitives::H256;
use sp_runtime::traits::{Convert, Hash};
use sp_std::prelude::*;

use totem_utils::record_type::RecordType;
use totem_utils::traits::{
    bonsai::Storing, orders::Validating as OrderValidating, teams::Validating as TeamsValidating,
    timekeeping::Validating as TimeValidating,
};
use totem_utils::{ok, StorageMapExt};

#[frame_support::pallet]
pub mod pallet {

    use super::*;

    #[pallet::pallet]
    #[pallet::generate_store(trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    #[pallet::getter(fn is_valid_record)]
    /// Bonsai Storage
    pub type IsValidRecord<T: Config> = StorageMap<_, Blake2_128Concat, T::Hash, T::Hash>;

    /* Hacky workaround for inability of RPC to query transaction by hash */

    #[pallet::storage]
    #[pallet::getter(fn is_started)]
    /// Maps to current block number allows interrogation of errors.
    pub type IsStarted<T: Config> = StorageMap<_, Blake2_128Concat, T::Hash, T::BlockNumber>;

    #[pallet::storage]
    #[pallet::getter(fn is_successful)]
    /// Future block number beyond which the Hash should deleted.
    pub type IsSuccessful<T: Config> = StorageMap<_, Blake2_128Concat, T::Hash, T::BlockNumber>;

    #[pallet::storage]
    #[pallet::getter(fn tx_list)]
    /// Tracking to ensure that we can perform housekeeping on finalization of block.
    pub type TxList<T: Config> = StorageMap<_, Blake2_128Concat, T::Hash, Vec<T::Hash>>;

    #[pallet::config] //TODO declare configs that are constant
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        // type Orders: OrderValidating<Self::AccountId,Self::Hash>;
        type Timekeeping: TimeValidating<Self::AccountId, Self::Hash>;
        type Projects: TeamsValidating<Self::AccountId, Self::Hash>;
        type Orders: OrderValidating<Self::AccountId, Self::Hash>;
        type BonsaiConversions: Convert<Self::Hash, H256>
            + Convert<Self::BlockNumber, u32>
            + Convert<u32, Self::BlockNumber>
            + Convert<H256, Self::Hash>;
    }

    #[pallet::error]
    pub enum Error<T> {}

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// This function stores a record hash for BONSAI 2FA for couchDB
        ///
        /// Record types are the same as the Archive Record Types
        /// * 3000 Activities (previously Projects)
        /// * 4000 Timekeeping
        /// * 5000 Orders
        ///
        #[pallet::weight(0/*TODO*/)]
        fn update_record(
            origin: OriginFor<T>,
            record_type: RecordType,
            key: T::Hash,
            bonsai_token: T::Hash,
        ) -> DispatchResultWithPostInfo {
            // check transaction signed
            let who = ensure_signed(origin)?;
            Self::check_remote_ownership(who.clone(), key.clone(), bonsai_token.clone(), record_type.clone())?;
            Self::insert_record(key.clone(), bonsai_token.clone())?;

            ok()
        }

        #[pallet::weight(0/*TODO*/)]
        fn on_finalize_example(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let _who = ensure_signed(origin)?;
            let current_block: T::BlockNumber = frame_system::Pallet::<T>::block_number();
            let current = <T::BonsaiConversions as Convert<T::BlockNumber, u32>>::convert(current_block);
            // Get all hashes
            let default_bytes = b"nobody can save fiat currency now";
            let list_key: T::Hash = T::Hashing::hash(default_bytes.encode().as_slice());
            if let Some(hashes) = Self::tx_list(&list_key) {
                // check which storage the hashes come from and hashes that are old
                for i in hashes {
                    let key: T::Hash = i.clone();
                    match Self::is_started(&key) {
                        Some(block) => {
                            let mut target_block =
                                <T::BonsaiConversions as Convert<T::BlockNumber, u32>>::convert(block);
                            target_block = target_block + 172800_u32;
                            // let mut target_deletion_block: T::BlockNumber = <T::BonsaiConversions as Convert<u32, T::BlockNumber>>::convert(target_block);
                            // cleanup 30 Days from when the transaction started, but did not complete
                            // It's possible this comparison is not working
                            if current >= target_block {
                                IsStarted::<T>::remove(key.clone());
                            }
                        }
                        None => {
                            if let Some(block) = Self::is_successful(&key) {
                                let target_block =
                                    <T::BonsaiConversions as Convert<T::BlockNumber, u32>>::convert(block);
                                if current >= target_block {
                                    IsSuccessful::<T>::remove(key.clone());
                                }
                            }
                        }
                    }
                    TxList::<T>::mutate_(&list_key, |tx_list| tx_list.retain(|v| v != &key));
                }
            }

            ok()
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ErrorRecordOwner(T::Hash),
        ErrorUnknownType(T::Hash),
    }
}

impl<T: Config> Pallet<T> {
    fn check_remote_ownership(o: T::AccountId, k: T::Hash, t: T::Hash, e: RecordType) -> DispatchResultWithPostInfo {
        // check which type of record
        // then check that the supplied hash is owned by the signer of the transaction
        match e {
            RecordType::Teams => {
                if let false = <<T as Config>::Projects as TeamsValidating<T::AccountId, T::Hash>>::is_project_owner(
                    o.clone(),
                    k.clone(),
                ) {
                    Self::deposit_event(Event::ErrorRecordOwner(t));
                    fail!("You cannot add a record you do not own");
                }
            }
            RecordType::Timekeeping => {
                if let false =
                    <<T as Config>::Timekeeping as TimeValidating<T::AccountId, T::Hash>>::is_time_record_owner(
                        o.clone(),
                        k.clone(),
                    )
                {
                    Self::deposit_event(Event::ErrorRecordOwner(t));
                    fail!("You cannot add a record you do not own");
                }
            }
            RecordType::Orders => {
                if let false = <<T as Config>::Orders as OrderValidating<T::AccountId, T::Hash>>::is_order_party(
                    o.clone(),
                    k.clone(),
                ) {
                    Self::deposit_event(Event::ErrorRecordOwner(t));
                    fail!("You cannot add a record you do not own");
                }
            }
        }
        ok()
    }

    fn insert_record(k: T::Hash, t: T::Hash) -> DispatchResultWithPostInfo {
        // TODO implement fee payment mechanism (currently just transaction fee)
        IsValidRecord::<T>::insert(k, t);
        ok()
    }

    fn insert_uuid(u: T::Hash) -> DispatchResultWithPostInfo {
        if IsSuccessful::<T>::contains_key(&u) {
            // Throw an error because the transaction already completed
            fail!("Queued transaction already completed");
        } else if IsStarted::<T>::contains_key(&u) {
            // What happens on error or second use

            // The transaction is now completed successfully update the state change
            // remove from started, and place in successful
            let current_block = <frame_system::Pallet<T>>::block_number();
            let mut block: u32 = T::BonsaiConversions::convert(current_block);
            block = block + 172800_u32; // cleanup in 30 Days
            let deletion_block: T::BlockNumber = <T::BonsaiConversions as Convert<u32, T::BlockNumber>>::convert(block);
            IsStarted::<T>::remove(&u);
            IsSuccessful::<T>::insert(u, deletion_block);
        } else {
            // this is a new UUID just starting the transaction
            let current_block = <frame_system::Pallet<T>>::block_number();
            let default_bytes = b"nobody can save fiat currency now";
            let list_key: T::Hash = T::Hashing::hash(default_bytes.encode().as_slice());
            TxList::<T>::mutate_(list_key, |tx_list| tx_list.push(u));
            IsStarted::<T>::insert(u, current_block);
        }

        ok()
    }
}

pub use pallet::*;

impl<T: Config> Storing<T::Hash> for Pallet<T> {
    fn claim_data(r: T::Hash, d: T::Hash) -> DispatchResultWithPostInfo {
        Self::insert_record(r, d)
    }

    fn store_uuid(u: T::Hash) -> DispatchResultWithPostInfo {
        Self::insert_uuid(u)
    }
}
