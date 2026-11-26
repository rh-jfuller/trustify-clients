use serde_json::json;
use trustify_client::api::types::{AdvisoryHead, Exploit};

#[test]
fn timestamps_accept_offsets_and_preserve_fractional_seconds() {
    let mut input = json!({
        "document_id": "advisory-1",
        "identifier": "ADV-1",
        "issuer": null,
        "labels": {},
        "published": "2026-11-24T13:34:56.123456789+01:00",
        "title": null,
        "uuid": "advisory-uuid",
        "withdrawn": null
    });
    let advisory: AdvisoryHead = serde_json::from_value(input.clone()).unwrap();
    let expected: jiff::Timestamp = "2026-11-24T12:34:56.123456789Z".parse().unwrap();
    assert_eq!(advisory.published, Some(expected));
    assert_eq!(advisory.modified, None);
    input["published"] = json!("2026-11-24T12:34:56.123456789Z");
    assert_eq!(serde_json::to_value(&advisory).unwrap(), input);
}

#[test]
fn date_only_fields_round_trip_with_nullable_dates() {
    let input = json!({
        "cve_id": "CVE-2026-1234",
        "date_reported": "2024-02-29",
        "id": "exploit-1",
        "metadata": {},
        "remediation_due_date": null,
        "source": "cisa-kev"
    });
    let exploit: Exploit = serde_json::from_value(input.clone()).unwrap();
    assert_eq!(
        exploit.date_reported,
        Some(jiff::civil::Date::new(2024, 2, 29).unwrap())
    );
    assert_eq!(exploit.remediation_due_date, None);
    assert_eq!(serde_json::to_value(exploit).unwrap(), input);
}
