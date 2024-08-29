use std::io::Read;
use csv::ReaderBuilder;
use serde::Deserialize;
use chrono::{DateTime, Utc, NaiveDateTime, Datelike};

// Electricity spot price format:
//   HourUTC;HourDK;PriceArea;SpotPriceDKK;SpotPriceEUR
//   2022-12-31 23:00;2023-01-01 00:00;DK1;14,950000;2,010000
#[derive(Debug, Deserialize)]
struct SpotPrice {
    #[serde(rename = "HourUTC", deserialize_with = "deserialize_timestamp_dk_hm")]
    hour_utc: DateTime<Utc>,
    #[serde(rename = "HourDK")]
    hour_dk: String,
    #[serde(rename = "PriceArea")]
    price_area: String,
    #[serde(rename = "SpotPriceDKK", deserialize_with = "deserialize_float")]
    spot_price_dkk: f64,
    #[serde(rename = "SpotPriceEUR", deserialize_with = "deserialize_float")]
    spot_price_eur: f64,
}

fn deserialize_float<'de, D>(deserializer: D) -> Result<f64, D::Error>
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

fn load_spot_prices<R: Read>(reader: R) -> csv::Result<Vec<SpotPrice>> {
    let mut rdr = ReaderBuilder::new()
        .delimiter(b';')
        .from_reader(reader);
    
    rdr.deserialize().collect()
}

// Meter data CSV format:
// MålepunktsID;Fra_dato;Til_dato;Mængde;Måleenhed;Kvalitet;Type
//      571313161100187650;01-01-2023 00:00:00;01-01-2023 01:00:00;0,11;KWH;Målt;Tidsserie

#[derive(Debug, Deserialize)]
struct MeterData {
    #[serde(rename = "MålepunktsID")]
    malepunkts_id: String,
    #[serde(rename = "Fra_dato", deserialize_with = "deserialize_timestamp_dk_hms")]
    fra_dato: DateTime<Utc>,
    #[serde(rename = "Til_dato", deserialize_with = "deserialize_timestamp_dk_hms")]
    til_dato: DateTime<Utc>,
    #[serde(rename = "Mængde", deserialize_with = "deserialize_float")]
    maengde: f64,
    #[serde(rename = "Måleenhed")]
    maleenhed: String,
    #[serde(rename = "Kvalitet")]
    kvalitet: String,
    #[serde(rename = "Type")]
    type_: String,
}

fn load_meter_data<R: Read>(reader: R) -> csv::Result<Vec<MeterData>> {
    let mut rdr = ReaderBuilder::new()
        .delimiter(b';')
        .from_reader(reader);
    
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


fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use chrono::{Utc, TimeZone};

    #[test]
    fn test_load_spot_prices() {
        let csv_data = "\
HourUTC;HourDK;PriceArea;SpotPriceDKK;SpotPriceEUR
2022-12-31 23:00;2023-01-01 00:00;DK1;14,950000;2,010000";

        let cursor = Cursor::new(csv_data);
        let spot_prices = load_spot_prices(cursor).unwrap();
        assert_eq!(spot_prices.len(), 1);
        let first_price = &spot_prices[0];
        assert_eq!(first_price.hour_utc, Utc.with_ymd_and_hms(2022, 12, 31, 23, 0, 0).unwrap());
        assert_eq!(first_price.hour_dk, "2023-01-01 00:00");
        assert_eq!(first_price.price_area, "DK1");
        assert_eq!(first_price.spot_price_dkk, 14.95);
        assert_eq!(first_price.spot_price_eur, 2.01);
    }

    #[test]
    fn test_load_meter_data() {
        let csv_data = "\
MålepunktsID;Fra_dato;Til_dato;Mængde;Måleenhed;Kvalitet;Type
571313161100187650;26-10-2023 11:00:00;26-10-2023 12:00:00;0,25;KWH;Målt;Tidsserie
571313161100187650;26-10-2023 12:00:00;26-10-2023 13:00:00;0,21;KWH;Målt;Tidsserie
571313161100187650;24-12-2023 12:00:00;24-12-2023 13:00:00;0,11;KWH;Målt;Tidsserie";

        let cursor = Cursor::new(csv_data);
        let meter_data = load_meter_data(cursor).unwrap();
        
        assert_eq!(meter_data.len(), 3);
        
        let first_entry = &meter_data[0];
        assert_eq!(first_entry.malepunkts_id, "571313161100187650");
        assert_eq!(first_entry.fra_dato, Utc.with_ymd_and_hms(2023, 10, 26, 9, 0, 0).unwrap());
        assert_eq!(first_entry.til_dato, Utc.with_ymd_and_hms(2023, 10, 26, 10, 0, 0).unwrap());
        assert_eq!(first_entry.maengde, 0.25);
        assert_eq!(first_entry.maleenhed, "KWH");
        assert_eq!(first_entry.kvalitet, "Målt");
        assert_eq!(first_entry.type_, "Tidsserie");

        let second_entry = &meter_data[1];
        assert_eq!(second_entry.malepunkts_id, "571313161100187650");
        assert_eq!(second_entry.fra_dato, Utc.with_ymd_and_hms(2023, 10, 26, 10, 0, 0).unwrap());
        assert_eq!(second_entry.til_dato, Utc.with_ymd_and_hms(2023, 10, 26, 11, 0, 0).unwrap());
        assert_eq!(second_entry.maengde, 0.21);
        assert_eq!(second_entry.maleenhed, "KWH");
        assert_eq!(second_entry.kvalitet, "Målt");
        assert_eq!(second_entry.type_, "Tidsserie");

        let second_entry = &meter_data[2];
        assert_eq!(second_entry.malepunkts_id, "571313161100187650");
        assert_eq!(second_entry.fra_dato, Utc.with_ymd_and_hms(2023, 12, 24, 11, 0, 0).unwrap());
        assert_eq!(second_entry.til_dato, Utc.with_ymd_and_hms(2023, 12, 24, 12, 0, 0).unwrap());
        assert_eq!(second_entry.maengde, 0.11);
        assert_eq!(second_entry.maleenhed, "KWH");
        assert_eq!(second_entry.kvalitet, "Målt");
        assert_eq!(second_entry.type_, "Tidsserie");
    }
}