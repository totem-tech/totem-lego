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

//! Various cross-pallet utils.

#![cfg_attr(not(feature = "std"), no_std)]

mod mock;
pub mod record_type;
pub mod traits;
pub mod types;

use codec::{Decode, Encode, EncodeLike, FullCodec, FullEncode, WrapperTypeEncode};
use frame_support::{dispatch::DispatchResultWithPostInfo, storage::StorageMap};

/// Easy return of an OK dispatch with no content.
pub fn ok() -> DispatchResultWithPostInfo {
    Ok(().into())
}

/// In addition to `StorageMap`, says if the mutation succeded.
pub enum Update {
    Done,
    KeyNotFound,
}

/// Adds behavior to `StorageMap`s.
pub trait StorageMapExt<K, V>
where
    Self: StorageMap<K, V>,
    K: FullEncode + Encode + EncodeLike,
    V: FullCodec + Decode + FullEncode + Encode + EncodeLike + WrapperTypeEncode,
{
    /// If the key exists in the map, modifies it with the provided function, and returns `Update::Done`.
    /// Otherwise, it does nothing and returns `Update::KeyNotFound`.
    fn mutate_<KeyArg: EncodeLike<K>, F: FnOnce(&mut V)>(key: KeyArg, f: F) -> Update {
        Self::mutate_exists(key, |option| match option.as_mut() {
            Some(value) => {
                f(value);
                Update::Done
            }
            None => Update::KeyNotFound,
        })
    }
}

impl<T, K, V> StorageMapExt<K, V> for T
where
    T: StorageMap<K, V>,
    K: FullEncode + Encode + EncodeLike,
    V: FullCodec + Decode + FullEncode + Encode + EncodeLike + WrapperTypeEncode,
{
}
