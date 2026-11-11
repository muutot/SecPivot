//! Owned command passwords are guarded during argument deserialization,
//! before an async command future can be cancelled without its first poll.

use serde::Deserialize;
use std::ops::Deref;
use zeroize::{Zeroize, Zeroizing};

/// String-shaped input with no Debug/Serialize implementation. The generic
/// payload permits safe destructor observation in the isolated command fixture;
/// only the default String payload implements IPC deserialization.
pub(crate) struct VaultPassword<T: Zeroize = String>(Zeroizing<T>);

impl<T: Zeroize> VaultPassword<T> {
    pub(crate) fn new(password: T) -> Self {
        Self(Zeroizing::new(password))
    }
}

impl<'de> Deserialize<'de> for VaultPassword {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Self::new)
    }
}

impl<T: Zeroize> Deref for VaultPassword<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: Zeroize> Zeroize for VaultPassword<T> {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}
