use std::convert::{TryFrom, TryInto};

use aes::Aes256;
use cipher::{BlockCipherEncrypt, KeyInit};
use hybrid_array::{typenum::U32, Array as GenericArray};
use sha2::{Digest, Sha256};

use super::CryptographyError;

pub(crate) trait Kdf {
    fn transform_key(
        &self,
        composite_key: &GenericArray<u8, U32>,
    ) -> Result<GenericArray<u8, U32>, CryptographyError>;
}

impl crate::config::KdfConfig {
    /// Measure this KDF using synthetic key material, through the same transform
    /// used to open/save a KDBX file. No database or user credential is needed.
    pub fn benchmark(&self) -> Result<std::time::Duration, CryptographyError> {
        let kdf = self.get_kdf_seeded(&[9_u8; 32]);
        let key = GenericArray::<u8, U32>::from([7_u8; 32]);
        let start = std::time::Instant::now();
        std::hint::black_box(kdf.transform_key(std::hint::black_box(&key))?);
        Ok(start.elapsed())
    }
}

pub struct AesKdf {
    pub seed: Vec<u8>,
    pub rounds: u64,
}

impl Kdf for AesKdf {
    #[allow(clippy::indexing_slicing)] // Slicing is safe because composite_key is always 32 bytes
    fn transform_key(
        &self,
        composite_key: &GenericArray<u8, U32>,
    ) -> Result<GenericArray<u8, U32>, CryptographyError> {
        let seed_arr = self
            .seed
            .as_slice()
            .try_into()
            .map_err(|_| CryptographyError::InvalidLength(cipher::InvalidLength))?;
        let cipher = Aes256::new(&seed_arr);
        let mut block1: GenericArray<u8, _> = composite_key[..16]
            .try_into()
            .map_err(|_| CryptographyError::InvalidLength(cipher::InvalidLength))?;
        let mut block2: GenericArray<u8, _> = composite_key[16..]
            .try_into()
            .map_err(|_| CryptographyError::InvalidLength(cipher::InvalidLength))?;
        for _ in 0..self.rounds {
            cipher.encrypt_block(&mut block1);
            cipher.encrypt_block(&mut block2);
        }

        let mut digest = Sha256::new();

        digest.update(block1);
        digest.update(block2);

        Ok(digest.finalize())
    }
}

pub struct Argon2Kdf {
    pub memory: u64,
    pub salt: Vec<u8>,
    pub iterations: u64,
    pub parallelism: u32,
    pub version: argon2::Version,
    pub variant: argon2::Variant,
}

impl Kdf for Argon2Kdf {
    fn transform_key(
        &self,
        composite_key: &GenericArray<u8, U32>,
    ) -> Result<GenericArray<u8, U32>, CryptographyError> {
        let mem_cost =
            u32::try_from(self.memory / 1024).map_err(|_| argon2::Error::MemoryTooMuch)?;
        let time_cost = u32::try_from(self.iterations).map_err(|_| argon2::Error::TimeTooLarge)?;
        // Disable Argon2 multithreading on wasm32 targets to avoid panics on platforms without thread support.
        let thread_mode = if cfg!(target_arch = "wasm32") {
            argon2::ThreadMode::Sequential
        } else {
            argon2::ThreadMode::Parallel
        };

        let config = argon2::Config {
            thread_mode,
            ad: &[],
            hash_length: 32,
            lanes: self.parallelism,
            mem_cost,
            secret: &[],
            time_cost,
            variant: self.variant,
            version: self.version,
        };

        let key = argon2::hash_raw(composite_key, &self.salt, &config)?;

        key.as_slice()
            .try_into()
            .map_err(|_| CryptographyError::InvalidLength(cipher::InvalidLength))
    }
}

#[cfg(test)]
mod tests {
    use argon2::{hash_raw, Config, ThreadMode, Variant, Version};
    use hybrid_array::{typenum::U32, Array as GenericArray};

    use super::{Argon2Kdf, Kdf};

    #[test]
    fn secpivot_benchmark_executes_the_real_transform_and_propagates_invalid_costs() {
        for config in [
            crate::config::KdfConfig::Aes { rounds: 1000 },
            crate::config::KdfConfig::Argon2 {
                iterations: 1,
                memory: 65536,
                parallelism: 1,
                version: Version::Version13,
            },
            crate::config::KdfConfig::Argon2id {
                iterations: 1,
                memory: 65536,
                parallelism: 1,
                version: Version::Version13,
            },
        ] {
            assert!(config.benchmark().unwrap() > std::time::Duration::ZERO);
        }
        let invalid = crate::config::KdfConfig::Argon2id {
            iterations: u64::from(u32::MAX) + 1,
            memory: 65536,
            parallelism: 1,
            version: Version::Version13,
        };
        assert!(invalid.benchmark().is_err());
    }

    #[test]
    fn secpivot_argon2_rejects_overflowing_iterations_before_derivation() {
        for variant in [Variant::Argon2d, Variant::Argon2id] {
            let kdf = Argon2Kdf {
                memory: 64 * 1024,
                salt: vec![9_u8; 32],
                iterations: (1u64 << 32) + 1,
                parallelism: 1,
                version: Version::Version13,
                variant,
            };
            let result = kdf.transform_key(&GenericArray::<u8, U32>::from([7_u8; 32]));
            assert!(matches!(
                result,
                Err(super::CryptographyError::Argon2(
                    argon2::Error::TimeTooLarge
                ))
            ));
        }
    }

    #[test]
    fn secpivot_argon2_rejects_overflowing_memory_before_derivation() {
        for variant in [Variant::Argon2d, Variant::Argon2id] {
            let kdf = Argon2Kdf {
                memory: ((1u64 << 32) + 64) * 1024,
                salt: vec![9_u8; 32],
                iterations: 1,
                parallelism: 1,
                version: Version::Version13,
                variant,
            };
            let result = kdf.transform_key(&GenericArray::<u8, U32>::from([7_u8; 32]));
            assert!(matches!(
                result,
                Err(super::CryptographyError::Argon2(
                    argon2::Error::MemoryTooMuch
                ))
            ));
        }
    }

    #[test]
    fn argon2_memory_is_interpreted_as_bytes() {
        let composite_key = GenericArray::<u8, U32>::from([7_u8; 32]);
        let salt = vec![9_u8; 32];
        let kdf = Argon2Kdf {
            memory: 64 * 1024,
            salt: salt.clone(),
            iterations: 1,
            parallelism: 1,
            version: Version::Version13,
            variant: Variant::Argon2id,
        };

        let expected = hash_raw(
            &composite_key,
            &salt,
            &Config {
                thread_mode: ThreadMode::Parallel,
                ad: &[],
                hash_length: 32,
                lanes: 1,
                mem_cost: 64,
                secret: &[],
                time_cost: 1,
                variant: Variant::Argon2id,
                version: Version::Version13,
            },
        )
        .expect("argon2 test vector should derive successfully");

        let actual = kdf
            .transform_key(&composite_key)
            .expect("argon2 kdf should derive successfully");

        assert_eq!(actual.as_slice(), expected.as_slice());
    }
}
