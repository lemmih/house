use chrono::{DateTime,  Utc};
use csv::ReaderBuilder;
use serde::Deserialize;
use std::io::Read;

// Irradiance data CSV format:
// time,P,G(i),H_sun,T2m,WS10m,Int
// 20200101:0011,0.0,0.0,0.0,3.79,7.31,0.0
#[derive(Debug, Deserialize)]
pub struct Irradiance {
    #[serde(rename = "time", deserialize_with = "deserialize_timestamp_radiation")]
    pub time: DateTime<Utc>,
    #[serde(rename = "P")]
    pub power: f64,
}

impl Irradiance {
    pub fn load_irradiance<R: Read>(reader: R) -> csv::Result<Vec<Irradiance>> {
        let mut rdr = ReaderBuilder::new()
            .delimiter(b',')
        .comment(Some(b'#'))
            .from_reader(reader);

        rdr.deserialize().collect()
    }
}

// Example: 20200101:0011
// Format is YYYYMMDD:HHMM
fn deserialize_timestamp_radiation<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = String::deserialize(deserializer)?;
    chrono::NaiveDateTime::parse_from_str(&s, "%Y%m%d:%H%M")
        .map(|naive| naive.and_utc())
        .map_err(serde::de::Error::custom)
}