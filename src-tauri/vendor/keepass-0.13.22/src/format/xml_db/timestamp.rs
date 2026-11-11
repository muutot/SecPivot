use base64::{engine::general_purpose as base64_engine, Engine as _};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize, Serializer};
use std::convert::TryInto;

/// In KDBX4, timestamps are stored as seconds, Base64 encoded, since 0001-01-01 00:00:00.
/// This function returns the epoch baseline used by KDBX for date serialization.
pub fn get_epoch_baseline() -> chrono::NaiveDateTime {
    #[allow(clippy::unwrap_used)] // This is a known constant, so unwrap is safe here.
    chrono::NaiveDateTime::parse_from_str("0001-01-01T00:00:00", "%Y-%m-%dT%H:%M:%S").unwrap()
}

#[derive(Debug, Clone, Copy)]
pub enum TimestampMode {
    Base64,
    Iso8601,
}

#[derive(Debug)]
pub struct Timestamp {
    pub mode: TimestampMode,
    pub time: NaiveDateTime,
}

impl Timestamp {
    pub fn new_base64(time: NaiveDateTime) -> Self {
        Timestamp {
            mode: TimestampMode::Base64,
            time,
        }
    }

    pub fn new_iso8601(time: NaiveDateTime) -> Self {
        Timestamp {
            mode: TimestampMode::Iso8601,
            time,
        }
    }
}

impl From<Timestamp> for NaiveDateTime {
    fn from(t: Timestamp) -> Self {
        t.time
    }
}

impl From<NaiveDateTime> for Timestamp {
    fn from(t: NaiveDateTime) -> Self {
        // always use base64 for serialization since that seems to be most compatible with keepass
        Timestamp::new_base64(t)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let t = String::deserialize(deserializer)?;

        match chrono::NaiveDateTime::parse_from_str(&t, "%Y-%m-%dT%H:%M:%SZ") {
            Ok(ndt) => Ok(Timestamp::new_iso8601(ndt)),

            // If we don't have a valid ISO 8601 string, assume we have found a Base64 encoded int.
            _ => {
                let v = base64_engine::STANDARD
                    .decode(t)
                    .map_err(serde::de::Error::custom)?;

                // A KDBX timestamp is exactly one little-endian 64-bit integer.
                let a: [u8; 8] = v
                    .try_into()
                    .map_err(|_| serde::de::Error::custom("Invalid KDBX timestamp length"))?;
                let ndt = chrono::Duration::try_seconds(i64::from_le_bytes(a))
                    .and_then(|duration| get_epoch_baseline().checked_add_signed(duration))
                    .ok_or_else(|| serde::de::Error::custom("KDBX timestamp is out of range"))?;

                Ok(Timestamp::new_base64(ndt))
            }
        }
    }
}

impl Serialize for Timestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.mode {
            TimestampMode::Iso8601 => {
                let s = self.time.format("%Y-%m-%dT%H:%M:%SZ").to_string();
                serializer.serialize_str(&s)
            }
            TimestampMode::Base64 => {
                let duration = self.time - get_epoch_baseline();
                let seconds = duration.num_seconds();
                let b = seconds.to_le_bytes();
                let b64 = base64_engine::STANDARD.encode(b);
                serializer.serialize_str(&b64)
            }
        }
    }
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize, Deserialize)]
    struct Test<T>(T);

    fn decode_text(text: String) -> Result<Timestamp, serde::de::value::Error> {
        Timestamp::deserialize(serde::de::value::StringDeserializer::new(text))
    }

    fn outcome(text: String) -> &'static str {
        match std::panic::catch_unwind(|| decode_text(text)) {
            Ok(Err(_)) => "error",
            Ok(Ok(_)) => "accepted",
            Err(_) => "panic",
        }
    }

    #[test]
    fn secpivot_timestamp_rejects_non_eight_byte_values_without_panicking() {
        let actual: Vec<_> = (0..8)
            .chain([9, 16])
            .map(|length| {
                (
                    length,
                    outcome(base64_engine::STANDARD.encode(vec![0; length])),
                )
            })
            .collect();
        assert!(
            actual.iter().all(|(_, result)| *result == "error"),
            "{:?}",
            actual
        );
    }

    #[test]
    fn secpivot_timestamp_rejects_unrepresentable_dates_without_panicking() {
        let epoch = get_epoch_baseline();
        let min = NaiveDateTime::MIN
            .signed_duration_since(epoch)
            .num_seconds();
        let max = NaiveDateTime::MAX
            .signed_duration_since(epoch)
            .num_seconds();
        let actual: Vec<_> = [i64::MIN, i64::MAX, min - 1, max + 1]
            .iter()
            .map(|seconds| outcome(base64_engine::STANDARD.encode(seconds.to_le_bytes())))
            .collect();
        assert!(
            actual.iter().all(|result| *result == "error"),
            "{:?}",
            actual
        );
    }

    #[test]
    fn secpivot_timestamp_keeps_supported_dates_and_encoding_errors() {
        let epoch = get_epoch_baseline();
        let min = NaiveDateTime::MIN
            .signed_duration_since(epoch)
            .num_seconds();
        let max = NaiveDateTime::MAX
            .signed_duration_since(epoch)
            .num_seconds();
        for seconds in [min, -1, 0, 1, 63_832_320_000, max] {
            let text = base64_engine::STANDARD.encode(seconds.to_le_bytes());
            let ts = decode_text(text.clone()).unwrap();
            assert_eq!(ts.time.signed_duration_since(epoch).num_seconds(), seconds);
            assert!(matches!(ts.mode, TimestampMode::Base64));
            assert_eq!(
                quick_xml::se::to_string(&Test(ts)).unwrap(),
                format!("<Test>{text}</Test>")
            );
        }
        let ts = decode_text("2023-10-05T12:34:56Z".to_owned()).unwrap();
        assert!(matches!(ts.mode, TimestampMode::Iso8601));
        assert_eq!(ts.time.to_string(), "2023-10-05 12:34:56");
        for invalid in ["!", "AQAAAA", "2023-13-05T12:34:56Z"] {
            assert_eq!(outcome(invalid.to_owned()), "error");
        }
    }

    #[cfg(feature = "save_kdbx4")]
    #[test]
    fn secpivot_timestamp_xml_reader_rejects_damage_and_keeps_native_roundtrip() {
        use crate::format::xml_db::{parse_xml, to_xml};
        let mut db = crate::Database::new();
        db.config.kdf_config = crate::config::KdfConfig::Aes { rounds: 1 };
        let expected = get_epoch_baseline() + chrono::Duration::seconds(1);
        db.root_mut().times.creation = Some(expected);
        let mut cipher = crate::config::InnerCipherConfig::Plain
            .get_cipher(&[])
            .unwrap();
        let (xml, attachments) = to_xml(&db, &mut *cipher).unwrap();
        let xml = String::from_utf8(xml).unwrap();
        let original = "<CreationTime>AQAAAAAAAAA=</CreationTime>";
        assert!(xml.contains(original));
        let mut actual = Vec::new();
        for text in [
            base64_engine::STANDARD.encode([0]),
            base64_engine::STANDARD.encode([0; 9]),
            base64_engine::STANDARD.encode(i64::MAX.to_le_bytes()),
        ] {
            let damaged =
                xml.replacen(original, &format!("<CreationTime>{text}</CreationTime>"), 1);
            actual.push(
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    parse_xml(damaged.as_bytes(), &attachments, &mut *cipher)
                })) {
                    Ok(Err(_)) => "error",
                    Ok(Ok(_)) => "accepted",
                    Err(_) => "panic",
                },
            );
        }
        // Controls execute even when one of the damaged inputs panics.
        let loaded = parse_xml(xml.as_bytes(), &attachments, &mut *cipher).unwrap();
        assert_eq!(loaded.root().times.creation, Some(expected));
        let key = crate::DatabaseKey::new().with_password("synthetic-only");
        let mut bytes = Vec::new();
        loaded.save(&mut bytes, key.clone()).unwrap();
        let reopened = crate::Database::parse(&bytes, key).unwrap();
        assert_eq!(reopened.root().times.creation, Some(expected));
        assert!(
            actual.iter().all(|result| *result == "error"),
            "{:?}",
            actual
        );
    }

    #[test]
    fn test_deserialize_timestamp_iso8601() {
        let ts_str = "2023-10-05T12:34:56Z";
        let ts: Timestamp = quick_xml::de::from_str(ts_str).unwrap();
        assert_eq!(
            ts.time,
            NaiveDateTime::parse_from_str("2023-10-05T12:34:56", "%Y-%m-%dT%H:%M:%S").unwrap()
        );
        match ts.mode {
            TimestampMode::Iso8601 => (),
            _ => panic!("Expected Iso8601 mode"),
        }
    }

    #[test]
    fn test_deserialize_timestamp_base64() {
        let ts_str = "AQAAAAAAAAA="; // Base64 for 1 second since epoch
        let ts: Timestamp = quick_xml::de::from_str(ts_str).unwrap();
        assert_eq!(ts.time, get_epoch_baseline() + chrono::Duration::seconds(1));
        match ts.mode {
            TimestampMode::Base64 => (),
            _ => panic!("Expected Base64 mode"),
        }
    }

    #[test]
    fn test_serialize_timestamp_iso8601() {
        let ts = Timestamp {
            mode: TimestampMode::Iso8601,
            time: NaiveDateTime::parse_from_str("2023-10-05T12:34:56", "%Y-%m-%dT%H:%M:%S")
                .unwrap(),
        };
        let serialized = quick_xml::se::to_string(&Test(ts)).unwrap();
        assert_eq!(serialized, "<Test>2023-10-05T12:34:56Z</Test>");
    }

    #[test]
    fn test_serialize_timestamp_base64() {
        let ts = Timestamp {
            mode: TimestampMode::Base64,
            time: get_epoch_baseline() + chrono::Duration::seconds(1),
        };

        let serialized = quick_xml::se::to_string(&Test(ts)).unwrap();
        assert_eq!(serialized, "<Test>AQAAAAAAAAA=</Test>");
    }
}
