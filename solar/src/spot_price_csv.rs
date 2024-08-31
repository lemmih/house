use chrono::{DateTime, Utc};
use csv::ReaderBuilder;
use serde::Deserialize;
use std::io::Read;

// Electricity spot price format:
//   HourUTC;HourDK;PriceArea;SpotPriceDKK;SpotPriceEUR
//   2022-12-31 23:00;2023-01-01 00:00;DK1;14,950000;2,010000
#[derive(Debug, Deserialize)]
pub struct SpotPrice {
    #[serde(rename = "HourUTC", deserialize_with = "deserialize_timestamp_dk_hm")]
    pub hour_utc: DateTime<Utc>,
    // #[serde(rename = "HourDK")]
    // hour_dk: String,
    #[serde(rename = "PriceArea")]
    pub price_area: String,
    #[serde(rename = "SpotPriceDKK", deserialize_with = "deserialize_float")]
    pub spot_price_dkk: f64,
    // #[serde(rename = "SpotPriceEUR", deserialize_with = "deserialize_float")]
    // spot_price_eur: f64,
}

impl SpotPrice {
    pub fn load_spot_prices<R: Read>(reader: R) -> csv::Result<Vec<SpotPrice>> {
        let mut rdr = ReaderBuilder::new().delimiter(b';').from_reader(reader);
        rdr.deserialize().collect()
    }
}

pub fn deserialize_float<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = String::deserialize(deserializer)?;
    let s = s.replace(',', "."); // Replace ',' with '.'
    s.parse::<f64>().map_err(serde::de::Error::custom) // Directly parse without locale
}

fn deserialize_timestamp_dk_hm<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = String::deserialize(deserializer)?;
    chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M")
        .map(|naive| naive.and_utc())
        .map_err(serde::de::Error::custom)
}
