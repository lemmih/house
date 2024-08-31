use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};
use std::collections::HashMap;

mod spot_price_csv;
use spot_price_csv::*;

mod meter_data_csv;
use meter_data_csv::*;

mod irradiance_csv;
use irradiance_csv::*;

// Percent
const SALES_TAX: f64 = 1.25;
// Krone per kWh
const ENERGY_TAX: f64 = 0.95;
// Krone per month
const FIXED_COST: f64 = 70.0;

// Solar size in kWp
const SOLAR_SIZE: f64 = 10.0;
// Battery size in kWh
const BATTERY_SIZE: f64 = 0.0;
const BATTERY_EFFICIENCY: f64 = 0.9;
const BATTERY_CHARGE_PERCENT: f64 = 0.8;

// tarrif in DKK/kWh
fn kwh_tarrif(date: DateTime<Utc>) -> f64 {
    // October to March: off peak 14.31, peak 42.93, super peak 128.78
    // April to September: off peak 14.31, peak 21.46, super peak 55.80
    // Off peak hours: 00:00 - 06:00
    // Peak hours: 06:00 - 17:00, 21:00 - 24:00
    // Super peak hours: 17:00 - 21:00
    let month = date.month();
    let hour = date.hour();

    let (off_peak, peak, super_peak) = if (4..=9).contains(&month) {
        // April to September
        (14.31, 21.46, 55.80)
    } else {
        // October to March
        (14.31, 42.93, 128.78)
    };

    let tarrif = match hour {
        0..=5 => off_peak,
        6..=16 | 21..=23 => peak,
        17..=20 => super_peak,
        _ => unreachable!("Invalid hour"),
    };
    tarrif / 100.0
}

struct Battery {
    capacity: f64, // kWh
    charge: f64, // kWh
}

impl Battery {

    fn new(capacity: f64) -> Self {
        Battery { capacity, charge: 0.0 }
    }

    // Charge the battery and return the kWh that couldn't be stored due to capacity limits
    fn charge(&mut self, charge: f64) -> f64 {
        let available_capacity = self.capacity - self.charge;
        let charge_amount = charge.min(available_capacity);
        self.charge += charge_amount;
        charge - charge_amount // Return excess charge
    }

    // Discharge the battery and return the kWh that couldn't be discharged due to low charge
    fn discharge(&mut self, discharge: f64) -> f64 {
        let discharge_amount = discharge.min(self.charge);
        self.charge -= discharge_amount;
        discharge - discharge_amount // Return excess discharge
    }
}

fn main() {
    let spot_prices_file =
        std::fs::File::open("Elspotprices.csv").expect("Failed to open Elspotprices.csv");
    let spot_prices: Vec<SpotPrice> = SpotPrice::load_spot_prices(spot_prices_file)
        .expect("Failed to load spot prices")
        .into_iter()
        .filter(|price| price.price_area == "DK2")
        .collect();
    // println!("Number of spot price entries: {}", spot_prices.len());

    let meter_data_file =
        std::fs::File::open("MeterData.csv").expect("Failed to open El_målinger.csv");
    let meter_data = load_meter_data(meter_data_file).expect("Failed to load meter data");
    // println!("Number of meter data entries: {}", meter_data.len());

    let irradiance_file =
        std::fs::File::open("Irradiance.csv").expect("Failed to open irradiance.csv");
    let irradiance = Irradiance::load_irradiance(irradiance_file).expect("Failed to load irradiance");
    // println!("Number of irradiance entries: {}", irradiance.len());

    // Create a HashMap to store spot prices for quick lookup
    let mut spot_price_map: HashMap<DateTime<Utc>, f64> = HashMap::new();
    for price in &spot_prices {
        spot_price_map.insert(price.hour_utc, price.spot_price_dkk);
    }

    // Create a HashMap to store meter data for quick lookup
    let mut meter_data_map: HashMap<DateTime<Utc>, f64> = HashMap::new();
    for data in &meter_data {
        meter_data_map.insert(data.fra_dato, data.maengde);
    }

    // Create a HashMap to store irradiance data for quick lookup
    let mut irradiance_map: HashMap<DateTime<Utc>, f64> = HashMap::new();
    for data in &irradiance {
        // Set the year in data.time to 2023
        let adjusted_time = data
            .time
            .with_year(2023)
            .unwrap_or(data.time)
            .with_minute(0)
            .unwrap()
            .with_second(0)
            .unwrap();
        irradiance_map.insert(adjusted_time, data.power);
    }

    // Iterate through each hour of 2023
    let start_date = Utc.with_ymd_and_hms(2023, 1, 1, 0, 0, 0).unwrap();
    let end_date = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
    let mut current_date = start_date;

    let mut battery = Battery::new(BATTERY_SIZE * BATTERY_CHARGE_PERCENT);

    let mut electricity_cost = FIXED_COST*12.0;
    let mut saved_dkk = 0.0;
    let mut saved_kwh = 0.0;
    let mut sold_profit = 0.0;
    let mut sold_kwh = 0.0;

    while current_date < end_date {
        let spot_price = spot_price_map.get(&current_date).unwrap_or(&0.0)/1000.0;
        let meter_reading = *meter_data_map.get(&current_date).unwrap_or(&0.0) * 3.0;
        let irradiance_value = irradiance_map.get(&current_date).unwrap_or(&0.0)/1000.0 * SOLAR_SIZE;

        let price_per_kwh = kwh_tarrif(current_date) + spot_price * SALES_TAX + ENERGY_TAX;

        let direct_used_solar = meter_reading.min(irradiance_value);
        let surplus_solar = irradiance_value - direct_used_solar;
        let grid_input = meter_reading - direct_used_solar;

        let sold_solar = battery.charge(surplus_solar * BATTERY_EFFICIENCY);
        let used_grid = battery.discharge(grid_input);
        let used_solar = meter_reading - used_grid;

        sold_profit += sold_solar * spot_price;
        electricity_cost += used_grid * price_per_kwh;
        saved_dkk += used_solar * price_per_kwh;
        saved_kwh += used_solar;
        sold_kwh += sold_solar;

        // if sold_solar > 0.00 {
        // println!("Date: {}, Spot Price: {:.4} DKK/kWh, Sold: {:.2} kWh, Profit: {:.2} DKK",
        //          current_date.format("%Y-%m-%d %H:%M"),
        //          spot_price,
        //          sold_solar,
        //          sold_solar * spot_price);
        // }
        
        current_date += chrono::Duration::hours(1);
    }
    println!("Total sold profit: {:.2} DKK", sold_profit);
    println!("Total electricity cost: {:.2} DKK", electricity_cost);
    println!("Total saved electricity cost: {:.2} DKK", saved_dkk);
    let total_benefit = sold_profit + saved_dkk ;
    println!("Total benefit (sold profit + saved cost): {:.2} DKK", total_benefit);
    println!("Total sold kWh: {:.2} kWh", sold_kwh);
    println!("Total saved kWh: {:.2} kWh", saved_kwh);
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use std::io::Cursor;

    #[test]
    fn test_load_spot_prices() {
        let csv_data = "\
HourUTC;HourDK;PriceArea;SpotPriceDKK;SpotPriceEUR
2022-12-31 23:00;2023-01-01 00:00;DK1;14,950000;2,010000";

        let cursor = Cursor::new(csv_data);
        let spot_prices = SpotPrice::load_spot_prices(cursor).unwrap();
        assert_eq!(spot_prices.len(), 1);
        let first_price = &spot_prices[0];
        assert_eq!(
            first_price.hour_utc,
            Utc.with_ymd_and_hms(2022, 12, 31, 23, 0, 0).unwrap()
        );
        // assert_eq!(first_price.hour_dk, "2023-01-01 00:00");
        assert_eq!(first_price.price_area, "DK1");
        assert_eq!(first_price.spot_price_dkk, 14.95);
        // assert_eq!(first_price.spot_price_eur, 2.01);
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
        // assert_eq!(first_entry.malepunkts_id, "571313161100187650");
        assert_eq!(
            first_entry.fra_dato,
            Utc.with_ymd_and_hms(2023, 10, 26, 9, 0, 0).unwrap()
        );
        // assert_eq!(first_entry.til_dato, Utc.with_ymd_and_hms(2023, 10, 26, 10, 0, 0).unwrap());
        assert_eq!(first_entry.maengde, 0.25);
        // assert_eq!(first_entry.maleenhed, "KWH");
        // assert_eq!(first_entry.kvalitet, "Målt");
        // assert_eq!(first_entry.type_, "Tidsserie");

        let second_entry = &meter_data[1];
        // assert_eq!(second_entry.malepunkts_id, "571313161100187650");
        assert_eq!(
            second_entry.fra_dato,
            Utc.with_ymd_and_hms(2023, 10, 26, 10, 0, 0).unwrap()
        );
        // assert_eq!(second_entry.til_dato, Utc.with_ymd_and_hms(2023, 10, 26, 11, 0, 0).unwrap());
        assert_eq!(second_entry.maengde, 0.21);
        // assert_eq!(second_entry.maleenhed, "KWH");
        // assert_eq!(second_entry.kvalitet, "Målt");
        // assert_eq!(second_entry.type_, "Tidsserie");

        let second_entry = &meter_data[2];
        // assert_eq!(second_entry.malepunkts_id, "571313161100187650");
        assert_eq!(
            second_entry.fra_dato,
            Utc.with_ymd_and_hms(2023, 12, 24, 11, 0, 0).unwrap()
        );
        // assert_eq!(second_entry.til_dato, Utc.with_ymd_and_hms(2023, 12, 24, 12, 0, 0).unwrap());
        assert_eq!(second_entry.maengde, 0.11);
        // assert_eq!(second_entry.maleenhed, "KWH");
        // assert_eq!(second_entry.kvalitet, "Målt");
        // assert_eq!(second_entry.type_, "Tidsserie");
    }

    #[test]
    fn test_load_irradiance() {
        let csv_data = "\
time,P,G(i),H_sun,T2m,WS10m,Int
20200101:0011,0.0,0.0,0.0,3.79,7.31,0.0";

        let cursor = Cursor::new(csv_data);
        let irradiance = Irradiance::load_irradiance(cursor).unwrap();
        assert_eq!(irradiance.len(), 1);
        let first_irradiance = &irradiance[0];
        assert_eq!(
            first_irradiance.time,
            Utc.with_ymd_and_hms(2020, 1, 1, 0, 11, 0).unwrap()
        );
        assert_eq!(first_irradiance.power, 0.0);
    }
}
