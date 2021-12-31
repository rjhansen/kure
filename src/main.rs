/*
   Copyright 2021, Rob Hansen.

   Licensed under the Apache License, Version 2.0 (the "License");
   you may not use this file except in compliance with the License.
   You may obtain a copy of the License at

       http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing, software
   distributed under the License is distributed on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
   See the License for the specific language governing permissions and
   limitations under the License.
 */

#[macro_use]
extern crate log;
extern crate env_logger;
extern crate glob;

use regex::Regex;
use self::glob::glob;
use std::path::Path;
use std::{io, thread, time};
use std::fs::File;
use std::io::{Read, Write};
use chrono::Utc;
use lazy_static::lazy_static;

struct SensorData {
    pub id: String,
    pub temperature: f32,
    pub timestamp: String,
}

fn get_sensor_dirs() -> Vec<String> {
    let base = "/sys/devices/w1_bus_master1/";
    let mut sensor_dirs: Vec<String> = Vec::new();

    // Create a list of directories where we might find sensor data.
    match glob(&(base.to_owned() + "28-*")) {
        Err(why) => error!("can't read sensor base dir: {}", why),
        Ok(entries) => for maybe_entry in entries {
            match maybe_entry {
                Err(why) => error!("bad entry in sensor dir: {}", why),
                Ok(entry) => {
                    match entry.is_dir() {
                        false => continue,
                        true => match entry.to_str() {
                            None => continue,
                            Some(n) => sensor_dirs.push(n.to_owned())
                        }
                    }
                }
            }
        }
    }
    sensor_dirs
}

fn get_sensor_data_from_file(filename: &str) -> Result<SensorData, &str> {
    lazy_static! {
        static ref IDRX : Regex = Regex::new(r#"^.*/28-([A-Fa-f0-9]+)/.*$"#).unwrap();
        static ref RX: Regex = Regex::new(r#"[a-fA-F0-9]{2}( [a-fA-F0-9]{2}){8} : crc=[a-fA-F0-9]{2} YES\r?\n([a-fA-F0-9]{2}(( [a-fA-F0-9]{2}){7})) [a-fA-F0-9]{2} t=(-?\d+)"#).unwrap();
    }
    let file = match File::open(filename) {
        Err(_) => return Err("couldn't open file"),
        Ok(n) => n
    };
    let mut record: String = String::new();
    match io::BufReader::new(file).read_to_string(&mut record) {
        Err(_) => return Err("couldn't read sensor data"),
        Ok(n) => n
    };

    let mut datum = SensorData {
        id: String::from("none"),
        temperature: -1001.0,
        timestamp: Utc::now().to_rfc3339()
    };

    for ident in IDRX.captures_iter(&filename) {
        datum.id = ident[1].to_string();
    }

    for cap in RX.captures_iter(&record) {
        datum.temperature = match cap[5].parse::<f32>() {
            Err(_) => return Err("not a valid temperature"),
            Ok(val) => val / 1000.0
        };
    }

    // If we have no valid data, err out.
    if datum.temperature < -100.0 || datum.temperature > 100.0 {
        Err("no valid data found") 
    } else {
        Ok(datum)
    }
}

fn get_sensor_files() -> Vec<String> {
    let mut sensor_files: Vec<String> = Vec::new();

    for dirname in get_sensor_dirs() {
        let path = Path::new(&dirname).join("w1_slave");
        if path.exists() && path.is_file() {
            let value = match path.to_str() {
                None => continue,
                Some(n) => n
            };
            sensor_files.push(value.to_string());
        }
    }

    sensor_files
}

fn get_sensor_data() -> Vec<SensorData> {
    let mut data: Vec<SensorData> = Vec::new();
    for filename in get_sensor_files() {
        match get_sensor_data_from_file(&filename) {
            Err(_) => continue,
            Ok(value) => data.push(value)
        };
    }
    data
}

fn make_json() -> String {
    let mut record_count = 0;
    let mut contents = "{".to_owned();
    for record in get_sensor_data() {
        record_count += 1;
        contents.push_str("\n  \"");
        contents.push_str(&record.id);
        contents.push_str("\": {\n    \"timestamp\": \"");
        contents.push_str(&record.timestamp);
        contents.push_str("\",\n    \"temperature\": ");
        contents.push_str(&format!("{}", record.temperature));
        contents.push_str("\n  },");
    }
    if record_count > 0 {
        contents.pop();
    }
    contents.push_str("\n}\n");
    contents
}

fn main() {
    env_logger::init();

    loop {
        let maybe_file = File::create("/tmp/kure.json");
        if maybe_file.is_ok() {
            let mut file = maybe_file.unwrap();
            match write!(file, "{}", make_json()) {
                Err(_) => error!("couldn't write to json"),
                Ok(_) => drop(file)
            };
        }
        thread::sleep(time::Duration::from_secs(30));
    }
}
