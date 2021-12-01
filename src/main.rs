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
extern crate daemonize;

use regex::Regex;
use self::glob::glob;
use std::path::{Path};
use std::{fs, io, thread, time};
use std::fs::File;
use std::io::{Seek, SeekFrom, Read, Write};
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use daemonize::Daemonize;

struct SensorData {
    pub id: String,
    pub temperature: f32,
    pub timestamp: String,
}

fn get_sensor_dirs() -> Vec<String> {
    // FIXME: once you have real hardware working, change the basedir
    // appropriately.
    let base = "/home/rjh/kure/";
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
        // FIXME: Does this regex work for below-zero temperatures?
        static ref RX: Regex = Regex::new(r#"[a-fA-F0-9]{2}( [a-fA-F0-9]{2}){8} : crc=[a-fA-F0-9]{2} YES\r?\n([a-fA-F0-9]{2}(( [a-fA-F0-9]{2}){7})) [a-fA-F0-9]{2} t=(\d+)"#).unwrap();
    }
    let mut file = match File::open(filename) {
        Err(_) => return Err("couldn't open file"),
        Ok(n) => n
    };
    let metadata = match fs::metadata(filename) {
        Err(_) => return Err("couldn't read metadata"),
        Ok(md) => md
    };
    if metadata.len() > 200 {
        // FIXME: race condition -- what if someone truncates the file 
        // as we're using it?
        // 
        // low priority, because it's unlikely to get exploited, but
        // really, I should look into it.
        file.seek(SeekFrom::End(200)).unwrap();
    }
    let mut last_200: String = String::new();
    match io::BufReader::new(file).read_to_string(&mut last_200) {
        Err(_) => return Err("couldn't read sensor data"),
        Ok(n) => n
    };

    let mut datum = SensorData {
        id: String::from("none"),
        temperature: -1001.0,
        timestamp: match metadata.modified() {
            Err(_) => {
                return Err("couldn't get timestamp");
            },
            Ok(n) => {
                let dt: DateTime<Utc> = n.clone().into();
                dt.to_rfc3339()
            }
        },
    };

    for cap in RX.captures_iter(&last_200) {
        datum.id = cap[2].to_string();
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

    // For each sensor directory, open the file "w1_slave" if it exists and
    // read the last 200 bytes looking for sensor data.
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
    let mut contents = "{".to_owned();
    for record in get_sensor_data() {
        contents.push_str("\n  \"");
        contents.push_str(&record.id);
        contents.push_str("\": {\n    \"timestamp\": \"");
        contents.push_str(&record.timestamp);
        contents.push_str("\",\n    \"temperature\": ");
        contents.push_str(&format!("{}", record.temperature));
        contents.push_str("\n  },");
    }
    contents.pop();
    contents.push_str("\n}");
    contents
}

fn main() {
    env_logger::init();

    let stdout = File::create("/tmp/kure.stdout").unwrap();
    let stderr = File::create("/tmp/kure.stderr").unwrap();
    match Daemonize::new()
        .working_directory("/tmp")
        .stdout(stdout)
        .stderr(stderr)
        .start() {
            Err(why) => error!("couldn't daemonize! {}", why),
            Ok(_) => println!("successfully started!")
        };

    loop {
        let maybe_file = File::create("kure.json");
        if maybe_file.is_ok() {
            let mut file = maybe_file.unwrap();
            match write!(file, "{}", make_json()) {
                Err(_) => error!("couldn't write to json"),
                Ok(_) => ()
            };
        }
        thread::sleep(time::Duration::from_secs(30));
    }
}
