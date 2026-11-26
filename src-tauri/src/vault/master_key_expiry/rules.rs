//! KeePass KeyUtil.HasKeyExpired: XML TimeSpan or XML UTC DateTime cutoff.
//! Durations use .NET's fixed 365-day years / 30-day remaining months and
//! truncate to 100ns ticks. DateTime fractions use ties-to-even tick rounding.
use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime, Offset, TimeZone};

const TICKS_PER_SECOND: i128 = 10_000_000;

pub(super) fn expired(
    changed: Option<NaiveDateTime>,
    rule: &str,
    now: NaiveDateTime,
    local_date: NaiveDate,
) -> Result<bool, ()> {
    let rule = rule.trim();
    if rule.is_empty() {
        return Ok(false);
    }
    if rule.starts_with(['P', 'p']) || rule.starts_with("-P") || rule.starts_with("-p") {
        let ticks = duration_ticks(rule)?;
        let Some(changed) = changed else {
            return Ok(false);
        };
        let deadline = changed
            .checked_add_signed(Duration::nanoseconds(ticks % 10_000_000 * 100))
            .and_then(|date| date.checked_add_signed(Duration::seconds(ticks / 10_000_000)))
            .filter(|date| (1..=9999).contains(&date.year()))
            .ok_or(())?;
        Ok(deadline < now)
    } else {
        let cutoff = cutoff(rule, local_date)?;
        Ok(changed.is_some_and(|changed| changed < cutoff))
    }
}

pub(super) fn duration_ticks(rule: &str) -> Result<i64, ()> {
    let (negative, text) = rule
        .strip_prefix('-')
        .map_or((false, rule), |text| (true, text));
    let text = text.strip_prefix('P').ok_or(())?;
    let mut values = [0i128; 6];
    let mut fraction = 0i128;
    let mut pos = 0;
    let bytes = text.as_bytes();
    let mut time = false;
    let mut last = None;
    let mut time_component = false;
    while pos < bytes.len() {
        if bytes[pos] == b'T' {
            if time {
                return Err(());
            }
            time = true;
            pos += 1;
            continue;
        }
        let start = pos;
        while pos < bytes.len() && bytes[pos].is_ascii_digit() {
            pos += 1;
        }
        let integer = if pos > start {
            text[start..pos].parse::<i32>().map_err(|_| ())?
        } else {
            0
        };
        let mut has_fraction = false;
        if bytes.get(pos) == Some(&b'.') {
            if !time {
                return Err(());
            }
            has_fraction = true;
            pos += 1;
            let start = pos;
            while pos < bytes.len() && bytes[pos].is_ascii_digit() {
                pos += 1;
            }
            // .NET permits both PT.5S and PT1.S, including PT.S as zero.
            let digits = &text[start..pos];
            for index in 0..7 {
                fraction = fraction * 10
                    + i128::from(digits.as_bytes().get(index).copied().unwrap_or(b'0') - b'0');
            }
        } else if pos == start {
            return Err(());
        }
        let index = match (time, bytes.get(pos)) {
            (false, Some(b'Y')) => 0,
            (false, Some(b'M')) => 1,
            (false, Some(b'D')) => 2,
            (true, Some(b'H')) => 3,
            (true, Some(b'M')) => 4,
            (true, Some(b'S')) => 5,
            _ => return Err(()),
        };
        if last.is_some_and(|last| index <= last) || (has_fraction && index != 5) {
            return Err(());
        }
        values[index] = i128::from(integer);
        last = Some(index);
        time_component |= time;
        pos += 1;
    }
    if last.is_none() || (time && !time_component) {
        return Err(());
    }
    let days = (values[0] + values[1] / 12) * 365 + values[1] % 12 * 30 + values[2];
    let seconds = ((days * 24 + values[3]) * 60 + values[4]) * 60 + values[5];
    let ticks = seconds * TICKS_PER_SECOND + fraction;
    i64::try_from(if negative { -ticks } else { ticks }).map_err(|_| ())
}

fn digits(text: &str, length: usize) -> Result<u32, ()> {
    if text.len() != length || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(());
    }
    text.parse().map_err(|_| ())
}

pub(super) fn cutoff(rule: &str, local_date: NaiveDate) -> Result<NaiveDateTime, ()> {
    cutoff_with_offset(rule, local_date, |date| {
        Local
            .offset_from_local_datetime(&date)
            .single()
            .map(|offset| i64::from(offset.fix().local_minus_utc()))
            .ok_or(())
    })
}

pub(super) fn cutoff_with_offset(
    rule: &str,
    local_date: NaiveDate,
    mut local_offset: impl FnMut(NaiveDateTime) -> Result<i64, ()>,
) -> Result<NaiveDateTime, ()> {
    if !rule.is_ascii() {
        return Err(());
    }
    let (text, offset, explicit_zone) = if rule.ends_with(['Z', 'z']) {
        (&rule[..rule.len() - 1], 0, false)
    } else if rule.len() >= 6
        && matches!(rule.as_bytes()[rule.len() - 6], b'+' | b'-')
        && rule.as_bytes()[rule.len() - 3] == b':'
    {
        let start = rule.len() - 6;
        let hours = digits(&rule[start + 1..start + 3], 2)?;
        let minutes = digits(&rule[start + 4..], 2)?;
        // XmlConvert's parser accepts 00..99 for each timezone component.
        let sign = if rule.as_bytes()[start] == b'+' {
            1
        } else {
            -1
        };
        (
            &rule[..start],
            sign * i64::from(hours * 3600 + minutes * 60),
            true,
        )
    } else {
        (rule, 0, false)
    };
    let (date, time) = if let Some((date, time)) = text.split_once('T') {
        if date.len() != 10 {
            return Err(());
        }
        (parse_date(date, local_date)?, Some(time))
    } else if text.contains(':') {
        (local_date, Some(text))
    } else {
        (parse_date(text, local_date)?, None)
    };
    let mut value = date.and_hms_opt(0, 0, 0).ok_or(())?;
    if let Some(time) = time {
        if time.len() < 8 || &time[2..3] != ":" || &time[5..6] != ":" {
            return Err(());
        }
        let hour = digits(&time[..2], 2)?;
        let minute = digits(&time[3..5], 2)?;
        let second = digits(&time[6..8], 2)?;
        value = date
            .and_hms_opt(hour, minute, second)
            .filter(|_| second < 60)
            .ok_or(())?;
        if time.len() > 8 {
            let fraction = time[8..].strip_prefix('.').ok_or(())?;
            if fraction.is_empty() || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(());
            }
            let mut ticks = 0i64;
            for index in 0..7 {
                ticks = ticks * 10
                    + i64::from(fraction.as_bytes().get(index).copied().unwrap_or(b'0') - b'0');
            }
            if fraction.len() > 7 {
                let round = fraction.as_bytes()[7] - b'0';
                if round > 5
                    || (round == 5
                        && (ticks % 2 != 0
                            || fraction.as_bytes()[8..].iter().any(|byte| *byte != b'0')))
                {
                    ticks += 1;
                }
            }
            value = value
                .checked_add_signed(Duration::nanoseconds(ticks * 100))
                .ok_or(())?;
        }
    }
    if !(1..=9999).contains(&value.year()) {
        return Err(());
    }
    let min = NaiveDate::from_ymd_opt(1, 1, 1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    let max = NaiveDate::from_ymd_opt(9999, 12, 31)
        .unwrap()
        .and_hms_nano_opt(23, 59, 59, 999_999_900)
        .unwrap();
    let utc = value
        .checked_sub_signed(Duration::seconds(offset))
        .ok_or(())?;
    // XmlConvert first converts explicit offsets to a Local DateTime, then
    // ToUniversalTime. Both conversions saturate at .NET's date bounds.
    // Ordinary dates need no local timezone lookup. Only a boundary crossing
    // can differ from a direct UTC conversion (e.g. year 9999 in UTC+8).
    if explicit_zone && (utc < min + Duration::days(5) || utc > max - Duration::days(5)) {
        let local = utc
            .checked_add_signed(Duration::seconds(local_offset(value)?))
            .ok_or(())?;
        let bounded = local.clamp(min, max);
        if local != bounded || utc < min || utc > max {
            return Ok(bounded
                .checked_sub_signed(Duration::seconds(local_offset(bounded)?))
                .ok_or(())?
                .clamp(min, max));
        }
    }
    Ok(utc.clamp(min, max))
}

fn parse_date(text: &str, local_date: NaiveDate) -> Result<NaiveDate, ()> {
    let (year, month, day) = match text.len() {
        10 if &text[4..5] == "-" && &text[7..8] == "-" => (
            digits(&text[..4], 4)? as i32,
            digits(&text[5..7], 2)?,
            digits(&text[8..], 2)?,
        ),
        7 if &text[4..5] == "-" && !text.starts_with('-') => {
            (digits(&text[..4], 4)? as i32, digits(&text[5..], 2)?, 1)
        }
        4 if !text.starts_with('-') => (digits(text, 4)? as i32, 1, 1),
        7 if text.starts_with("--") && &text[4..5] == "-" => {
            (1904, digits(&text[2..4], 2)?, digits(&text[5..], 2)?)
        }
        4 if text.starts_with("--") => (local_date.year(), digits(&text[2..], 2)?, 1),
        6 if text.starts_with("--") && text.ends_with("--") => {
            (local_date.year(), digits(&text[2..4], 2)?, 1)
        }
        5 if text.starts_with("---") => (local_date.year(), 1, digits(&text[3..], 2)?),
        _ => return Err(()),
    };
    if !(1..=9999).contains(&year) {
        return Err(());
    }
    NaiveDate::from_ymd_opt(year, month, day).ok_or(())
}
