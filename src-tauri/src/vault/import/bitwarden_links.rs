//! Official Bitwarden linked IDs and literal import-time values.
//! Native imports additionally bind the allocated same-entry target names.

use serde_json::Value;

pub(super) fn resolve(item: &Value, kind: i64, linked_id: Option<&Value>) -> Result<String, ()> {
    let id = linked_id.and_then(Value::as_i64).ok_or(())?;
    let (section, key) = target(kind, id)?;
    let values = match item.get(section) {
        None | Some(Value::Null) => return Ok(String::new()),
        Some(Value::Object(values)) => values,
        _ => return Err(()),
    };
    let read = |key: &str| match values.get(key) {
        None | Some(Value::Null) => Ok(""),
        Some(Value::String(value)) => Ok(value.as_str()),
        _ => Err(()),
    };
    if key != "fullName" {
        return read(key).map(str::to_owned);
    }
    let parts = ["title", "firstName", "middleName", "lastName"]
        .into_iter()
        .map(read)
        .collect::<Result<Vec<_>, _>>()?;
    let name = parts
        .into_iter()
        .filter(|value| !js_trim(value).is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    Ok(js_trim(&name).to_owned())
}

pub(super) fn target(kind: i64, id: i64) -> Result<(&'static str, &'static str), ()> {
    let (section, key) = match (kind, id) {
        (1, 100) => ("login", "username"),
        (1, 101) => ("login", "password"),
        (3, 300..=305) => (
            "card",
            [
                "cardholderName",
                "expMonth",
                "expYear",
                "code",
                "brand",
                "number",
            ][(id - 300) as usize],
        ),
        (4, 400..=417) => (
            "identity",
            [
                "title",
                "middleName",
                "address1",
                "address2",
                "address3",
                "city",
                "state",
                "postalCode",
                "country",
                "company",
                "email",
                "phone",
                "ssn",
                "username",
                "passportNumber",
                "licenseNumber",
                "firstName",
                "lastName",
            ][(id - 400) as usize],
        ),
        (4, 418) => ("identity", "fullName"),
        _ => return Err(()),
    };
    Ok((section, key))
}

fn js_trim(value: &str) -> &str {
    value.trim_matches(|ch: char| (ch.is_whitespace() && ch != '\u{0085}') || ch == '\u{feff}')
}
