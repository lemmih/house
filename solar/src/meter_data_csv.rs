use chrono::{DateTime, Datelike, Utc};
use csv::ReaderBuilder;
use serde::Deserialize;
use std::io::Read;

// Meter data CSV format:
// MålepunktsID;Fra_dato;Til_dato;Mængde;Måleenhed;Kvalitet;Type
//      571313161100187650;01-01-2023 00:00:00;01-01-2023 01:00:00;0,11;KWH;Målt;Tidsserie

#[derive(Debug, Deserialize)]
pub struct MeterData {
    // #[serde(rename = "MålepunktsID")]
    // malepunkts_id: String,
    #[serde(rename = "Fra_dato", deserialize_with = "deserialize_timestamp_dk_hms")]
    pub fra_dato: DateTime<Utc>,
    // #[serde(rename = "Til_dato", deserialize_with = "deserialize_timestamp_dk_hms")]
    // til_dato: DateTime<Utc>,
    #[serde(rename = "Mængde", deserialize_with = "crate::spot_price_csv::deserialize_float")]
    pub maengde: f64,
    // #[serde(rename = "Måleenhed")]
    // maleenhed: String,
    // #[serde(rename = "Kvalitet")]
    // kvalitet: String,
    // #[serde(rename = "Type")]
    // type_: String,
}

pub fn load_meter_data<R: Read>(reader: R) -> csv::Result<Vec<MeterData>> {
    let mut rdr = ReaderBuilder::new().delimiter(b';').from_reader(reader);

    rdr.deserialize().collect()
}

// Parse date in format DD-MM-YYYY HH:MM:SS. The date is in Danish timezone.
// This means CEST (UTC+2) from last Sunday in March to last Sunday in October,
// and CET (UTC+1) otherwise.
// Note: This is not quite correct. It'll be wrong during the switch from CEST to CET.
fn deserialize_timestamp_dk_hms<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = String::deserialize(deserializer)?;
    let naive = chrono::NaiveDateTime::parse_from_str(&s, "%d-%m-%Y %H:%M:%S")
        .map_err(serde::de::Error::custom)?;

    let year = naive.year();

    // Compute last Sunday in March
    let last_sunday_march = chrono::NaiveDate::from_ymd_opt(year, 3, 31)
        .unwrap()
        .iter_days()
        .rev()
        .find(|d| d.weekday() == chrono::Weekday::Sun)
        .unwrap()
        .and_hms_opt(2, 0, 0)
        .unwrap();

    // Compute last Sunday in October
    let last_sunday_october = chrono::NaiveDate::from_ymd_opt(year, 10, 31)
        .unwrap()
        .iter_days()
        .rev()
        .find(|d| d.weekday() == chrono::Weekday::Sun)
        .unwrap()
        .and_hms_opt(2, 0, 0)
        .unwrap();

    let adjusted_naive = if naive >= last_sunday_march && naive < last_sunday_october {
        naive - chrono::Duration::hours(2)
    } else {
        naive - chrono::Duration::hours(1)
    };
    Ok(adjusted_naive.and_utc())
}