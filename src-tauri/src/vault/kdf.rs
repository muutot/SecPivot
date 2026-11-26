//! Exact KDBX cost parameters and measured, credential-free calibration.
use keepass::config::KdfConfig;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// u64 values travel as decimal strings to avoid JavaScript rounding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "algorithm",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum KdfParameters {
    Aes {
        rounds: String,
    },
    Argon2 {
        iterations: String,
        memory_bytes: String,
        parallelism: u32,
        version: u32,
    },
    Argon2id {
        iterations: String,
        memory_bytes: String,
        parallelism: u32,
        version: u32,
    },
}

fn cost(value: &str) -> Result<u64, String> {
    if value.is_empty() || value.len() > 20 || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err("KDF parameters must be unsigned decimal integers".into());
    }
    value
        .parse()
        .map_err(|_| "KDF parameter exceeds u64".into())
}

impl KdfParameters {
    pub(crate) fn algorithm(&self) -> &'static str {
        match self {
            Self::Aes { .. } => "Aes",
            Self::Argon2 { .. } => "Argon2",
            Self::Argon2id { .. } => "Argon2id",
        }
    }

    pub(crate) fn from_config(config: &KdfConfig) -> Result<Self, String> {
        match config {
            KdfConfig::Aes { rounds } => Ok(Self::Aes {
                rounds: rounds.to_string(),
            }),
            KdfConfig::Argon2 {
                iterations,
                memory,
                parallelism,
                version,
            } => Ok(Self::Argon2 {
                iterations: iterations.to_string(),
                memory_bytes: memory.to_string(),
                parallelism: *parallelism,
                version: version.as_u32(),
            }),
            KdfConfig::Argon2id {
                iterations,
                memory,
                parallelism,
                version,
            } => Ok(Self::Argon2id {
                iterations: iterations.to_string(),
                memory_bytes: memory.to_string(),
                parallelism: *parallelism,
                version: version.as_u32(),
            }),
            _ => Err("Unsupported KDF algorithm".into()),
        }
    }

    pub(crate) fn to_config(&self) -> Result<KdfConfig, String> {
        match self {
            Self::Aes { rounds } => {
                let rounds = cost(rounds)?;
                Ok(KdfConfig::Aes { rounds })
            }
            Self::Argon2 {
                iterations,
                memory_bytes,
                parallelism,
                version,
            }
            | Self::Argon2id {
                iterations,
                memory_bytes,
                parallelism,
                version,
            } => {
                let iterations = cost(iterations)?;
                let memory = cost(memory_bytes)?;
                if iterations == 0
                    || iterations > u64::from(u32::MAX)
                    || *parallelism == 0
                    || *parallelism > 0x00ff_ffff
                    || memory / 1024 < u64::from(*parallelism) * 8
                    || memory / 1024 > u64::from(u32::MAX)
                {
                    return Err("Invalid Argon2 cost parameters".into());
                }
                let version = match version {
                    0x10 => argon2::Version::Version10,
                    0x13 => argon2::Version::Version13,
                    _ => return Err("Unsupported Argon2 version".into()),
                };
                if matches!(self, Self::Argon2 { .. }) {
                    Ok(KdfConfig::Argon2 {
                        iterations,
                        memory,
                        parallelism: *parallelism,
                        version,
                    })
                } else {
                    Ok(KdfConfig::Argon2id {
                        iterations,
                        memory,
                        parallelism: *parallelism,
                        version,
                    })
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KdfBenchmarkResult {
    pub parameters: KdfParameters,
    pub elapsed_ms: f64,
    pub target_ms: u32,
    pub samples: u32,
    pub target_reached: bool,
}

fn set_work(config: &mut KdfConfig, work: u64) -> Result<(), String> {
    match config {
        KdfConfig::Aes { rounds } => *rounds = work,
        KdfConfig::Argon2 { iterations, .. } | KdfConfig::Argon2id { iterations, .. } => {
            *iterations = work
        }
        _ => return Err("Unsupported KDF algorithm".into()),
    }
    Ok(())
}

/// Every returned suggestion has actually been measured, including the final
/// candidate. Memory/lanes/version stay fixed; only transform work is calibrated.
pub(crate) fn benchmark_kdf(
    parameters: &KdfParameters,
    target_ms: u32,
) -> Result<KdfBenchmarkResult, String> {
    calibrate(parameters, target_ms, |config| {
        config
            .benchmark()
            .map_err(|_| "KDF benchmark failed".to_owned())
    })
}

fn calibrate(
    parameters: &KdfParameters,
    target_ms: u32,
    mut measure: impl FnMut(&KdfConfig) -> Result<Duration, String>,
) -> Result<KdfBenchmarkResult, String> {
    if !(100..=5000).contains(&target_ms) {
        return Err("KDF benchmark target must be 100–5000 ms".into());
    }
    let mut config = parameters.to_config()?;
    if let KdfConfig::Argon2 {
        memory,
        parallelism,
        ..
    }
    | KdfConfig::Argon2id {
        memory,
        parallelism,
        ..
    } = &config
    {
        if *memory > 256 * 1024 * 1024 || *parallelism > 64 {
            return Err("KDF benchmark supports up to 256 MiB and 64 lanes".into());
        }
    }
    let max_work = if matches!(config, KdfConfig::Aes { .. }) {
        u64::MAX
    } else {
        u64::from(u32::MAX)
    };
    let mut work: u64 = if matches!(config, KdfConfig::Aes { .. }) {
        10_000
    } else {
        1
    };
    let target_ns = u128::from(target_ms) * 1_000_000;
    let start = Instant::now();
    let mut result = None;
    for samples in 1..=8 {
        set_work(&mut config, work)?;
        let elapsed = measure(&config)?;
        let ns = elapsed.as_nanos().max(1);
        let reached = ns.abs_diff(target_ns) <= target_ns * 15 / 100;
        result = Some(KdfBenchmarkResult {
            parameters: KdfParameters::from_config(&config)?,
            elapsed_ms: elapsed.as_secs_f64() * 1000.0,
            target_ms,
            samples,
            target_reached: reached,
        });
        let proposed = (u128::from(work) * target_ns / ns).clamp(1, u128::from(max_work));
        let next = u64::try_from(proposed)
            .map_err(|_| "KDF benchmark cost overflow".to_owned())?
            .min(work.saturating_mul(4));
        if reached || next == work || start.elapsed() >= Duration::from_secs(20) {
            break;
        }
        work = next;
    }
    result.ok_or_else(|| "KDF benchmark produced no samples".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argon() -> KdfParameters {
        KdfParameters::Argon2id {
            iterations: "3".into(),
            memory_bytes: "65537".into(),
            parallelism: 2,
            version: 16,
        }
    }

    #[test]
    fn exact_u64_costs_and_argon_bytes_survive_wire_projection() {
        for rounds in [0, (1u64 << 53) + 1, u64::MAX] {
            let config = KdfConfig::Aes { rounds };
            let params = KdfParameters::from_config(&config).unwrap();
            let wire = serde_json::to_string(&params).unwrap();
            let decoded: KdfParameters = serde_json::from_str(&wire).unwrap();
            assert_eq!(decoded.to_config().unwrap(), config);
        }
        let params = argon();
        assert_eq!(
            KdfParameters::from_config(&params.to_config().unwrap()).unwrap(),
            params
        );
    }

    #[test]
    fn malformed_costs_never_become_reduced_costs() {
        for rounds in [
            "",
            "-1",
            "1.5",
            "1e5",
            "18446744073709551616",
            " 123",
            "+123",
        ] {
            assert!(KdfParameters::Aes {
                rounds: rounds.into()
            }
            .to_config()
            .is_err());
        }
        for wire in [
            r#"{"algorithm":"Aes","rounds":9007199254740993}"#,
            r#"{"algorithm":"Aes","rounds":"100","memoryBytes":"1024"}"#,
            r#"{"algorithm":"Aes","rounds":"100","rounds":"200"}"#,
            r#"{"algorithm":"Other","rounds":"100"}"#,
        ] {
            assert!(serde_json::from_str::<KdfParameters>(wire).is_err());
        }
        for params in [
            KdfParameters::Argon2id {
                iterations: "4294967296".into(),
                memory_bytes: "65536".into(),
                parallelism: 1,
                version: 19,
            },
            KdfParameters::Argon2id {
                iterations: "1".into(),
                memory_bytes: "4398046511104".into(),
                parallelism: 1,
                version: 19,
            },
            KdfParameters::Argon2id {
                iterations: "1".into(),
                memory_bytes: "8192".into(),
                parallelism: 2,
                version: 19,
            },
        ] {
            assert!(params.to_config().is_err());
        }
    }

    #[test]
    fn calibration_measures_the_final_suggestion_and_does_not_run_imported_huge_rounds() {
        let params = KdfParameters::Aes {
            rounds: u64::MAX.to_string(),
        };
        let mut observed = vec![];
        let result = calibrate(&params, 500, |config| {
            let KdfConfig::Aes { rounds } = config else {
                panic!("expected AES")
            };
            observed.push(*rounds);
            Ok(Duration::from_micros(*rounds * 10))
        })
        .unwrap();
        assert_eq!(observed, vec![10_000, 40_000, 50_000]);
        assert_eq!(
            result.parameters,
            KdfParameters::Aes {
                rounds: "50000".into()
            }
        );
        assert_eq!(result.elapsed_ms, 500.0);
        assert_eq!(result.samples, 3);
        assert!(result.target_reached);
    }

    #[test]
    fn calibration_preserves_argon_memory_parallelism_and_version() {
        let mut observed = vec![];
        let result = calibrate(&argon(), 400, |config| {
            let KdfConfig::Argon2id {
                iterations,
                memory,
                parallelism,
                version,
            } = config
            else {
                panic!("expected Argon2id")
            };
            assert_eq!(
                (*memory, *parallelism, *version),
                (65537, 2, argon2::Version::Version10)
            );
            observed.push(*iterations);
            Ok(Duration::from_millis(*iterations * 100))
        })
        .unwrap();
        assert_eq!(observed, [1, 4]);
        assert_eq!(result.elapsed_ms, 400.0);
        assert!(result.target_reached);
    }

    #[test]
    fn minimum_cost_and_errors_do_not_claim_a_successful_target() {
        let result = calibrate(&argon(), 100, |_| Ok(Duration::from_millis(200))).unwrap();
        assert_eq!(result.samples, 1);
        assert!(!result.target_reached);
        assert_eq!(result.elapsed_ms, 200.0);
        assert!(calibrate(&argon(), 100, |_| Err("synthetic failure".into())).is_err());
        assert!(calibrate(&argon(), 99, |_| panic!("must reject before measurement")).is_err());
        let oversized = KdfParameters::Argon2id {
            iterations: "1".into(),
            memory_bytes: "268435457".into(),
            parallelism: 1,
            version: 19,
        };
        assert!(calibrate(&oversized, 100, |_| panic!("must reject before allocation")).is_err());
    }
}
