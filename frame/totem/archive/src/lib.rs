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

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{dispatch::EncodeLike, fail, pallet_prelude::*};
use frame_system::pallet_prelude::*;

use sp_std::prelude::*;

use totem_utils::record_type::RecordType;
use totem_utils::traits::timekeeping::Validating as TimeValidating;

type Archival = bool;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {

    use super::*;

    #[pallet::pallet]
    #[pallet::generate_store(trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::config] //TODO declare configs that are constant
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Timekeeping: TimeValidating<Self::AccountId, Self::Hash>;
    }

    #[pallet::error]
    pub enum Error<T> {}

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(0/*TODO*/)]
        fn archive_record(
            origin: OriginFor<T>,
            record_type: RecordType,
            bonsai_token: T::Hash,
            archive: bool,
        ) -> DispatchResultWithPostInfo {
            // check signed
            let who = ensure_signed(origin)?;
            // check which type of record
            match record_type {
                RecordType::Timekeeping => {
                    // module specific archive handling
                    if T::Timekeeping::validate_and_archive(who.clone(), bonsai_token, archive) {
                        // issue event
                        Self::deposit_event(Event::RecordArchived(RecordType::Timekeeping, who, bonsai_token, archive));
                    }
                }
                _ => fail!("Unknown or unimplemented record type. Cannot archive record"),
            }

            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        RecordArchived(RecordType, T::AccountId, T::Hash, Archival),
    }
}
