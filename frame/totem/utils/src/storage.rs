//! Adds behavior to `StorageMap`s.

use codec::{Decode, Encode, EncodeLike, FullCodec, FullEncode, WrapperTypeEncode};
use frame_support::storage::StorageMap;

pub trait StorageMapExt<K, V>
where
    Self: StorageMap<K, V>,
    K: FullEncode + Encode + EncodeLike,
    V: FullCodec + Decode + FullEncode + Encode + EncodeLike + WrapperTypeEncode,
{
    /// If the key exists in the map, modifies it with the provided function, and returns `Ok`.
    /// Otherwise, it returns the given error.
    fn mutate_or_err<KeyArg, F, E>(key: KeyArg, f: F, err: E) -> Result<(), E>
    where
        KeyArg: EncodeLike<K>,
        F: FnOnce(&mut V),
    {
        Self::mutate_exists(key, |option| match option.as_mut() {
            Some(value) => {
                f(value);
                Ok(())
            }
            None => Err(err),
        })
    }

    /// If the key isn't found, insert first a default value before mutating it.
    fn mutate_default<KeyArg, F>(key: KeyArg, f: F)
    where
        KeyArg: EncodeLike<K>,
        F: FnOnce(&mut V),
        V: Default,
    {
        Self::mutate_exists(key, |option| {
            let value = match option {
                Some(value) => value,
                slot => {
                    *slot = Some(Default::default());
                    slot.as_mut().unwrap()
                }
            };
            f(value);
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
