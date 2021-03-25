// Copyright (c) 2021 Michael J. Simms. All rights reserved.

extern crate gpx;
extern crate serde;
extern crate serde_json;
extern crate tcx;

mod utils;
mod cadence_analyzer;
mod location_analyzer;
mod power_analyzer;
mod heart_rate_analyzer;

use wasm_bindgen::prelude::*;
use std::io::BufReader;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
extern {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn greet() {
    alert("Copyright (c) 2021 Michael J. Simms. All rights reserved.");
}

fn make_final_report(analyzer: &location_analyzer::LocationAnalyzer, power_analyzer: Option<&power_analyzer::PowerAnalyzer>, cadence_analyzer: Option<&cadence_analyzer::CadenceAnalyzer>) -> String {
    let mut max_power = 0.0;
    let mut avg_power = 0.0;
    let mut power_readings = Vec::<f64>::new();
    let mut max_cadence = 0.0;
    let mut avg_cadence = 0.0;
    let mut cadence_readings = Vec::<f64>::new();

    match power_analyzer {
        None => {
        }
        Some(power_analyzer) => {
            max_power = power_analyzer.max_power;
            avg_power = power_analyzer.compute_average();
            //power_readings = power_analyzer.readings;
        }
    }

    match cadence_analyzer {
        None => {
        }
        Some(cadence_analyzer) => {
            max_cadence = cadence_analyzer.max_cadence;
            avg_cadence = cadence_analyzer.compute_average();
            //cadence_readings = cadence_analyzer.readings;
        }
    }

    let analysis_report_str = serde_json::json!({
        "Start Time (ms)": analyzer.start_time_ms,
        "End Time (ms)": analyzer.last_time_ms,
        "Elapsed Time": (analyzer.last_time_ms - analyzer.start_time_ms) / 1000,
        "Total Distance": analyzer.total_distance,
        "Total Vertical Distance": analyzer.total_vertical,
        "Average Speed": analyzer.avg_speed,
        "Best 1K": analyzer.get_best_time(location_analyzer::BEST_1K),
        "Best Mile": analyzer.get_best_time(location_analyzer::BEST_MILE),
        "Best 5K": analyzer.get_best_time(location_analyzer::BEST_5K),
        "Best 10K": analyzer.get_best_time(location_analyzer::BEST_10K),
        "Best 15K": analyzer.get_best_time(location_analyzer::BEST_15K),
        "Best Half Marathon": analyzer.get_best_time(location_analyzer::BEST_HALF_MARATHON),
        "Best Marathon": analyzer.get_best_time(location_analyzer::BEST_MARATHON),
        "Mile Splits": analyzer.mile_splits,
        "KM Splits": analyzer.km_splits,
        "Times": analyzer.speed_times,
        "Speeds": analyzer.speed_graph,
        "Maximum Power": max_power,
        "Average Power": avg_power,
        "Power Readings": power_readings,
        "Maximum Cadence": max_cadence,
        "Average Cadence": avg_cadence,
        "Cadence Readings": cadence_readings
    }).to_string();

    analysis_report_str
}

#[wasm_bindgen]
pub fn analyze_gpx(s: &str) -> String {
    let mut analysis_report_str = String::new();

    let data = BufReader::new(s.as_bytes());
    let res = gpx::read(data);

    match res {
        Ok(gpx) => {
            let mut analyzer = location_analyzer::LocationAnalyzer::new();

            // Iterate through the tracks.
            for track in gpx.tracks {

                // Get the track name.
                match &track._type {
                    Some(activity_type) => analyzer.set_activity_type(activity_type.to_string()),
                    _ => {},
                }

                // Iterate through the track segments.
                for trackseg in track.segments {

                    // Iterate through the points.
                    for point in trackseg.points {
                        let time = point.time.unwrap().timestamp();
                        let lat = point.point().y();
                        let lon = point.point().x();
                        let alt = point.elevation.unwrap();

                        analyzer.append_location((time * 1000) as u64, lat, lon, alt);
                        analyzer.update_speeds();
                    }
                }
            }

            // For calculations that only make sense once all the points have been added.
            analyzer.analyze();

            // Copy items to the final report.
            analysis_report_str = make_final_report(&analyzer, None, None);
        }
        Err(_e) => {
            alert("Error parsing GPX file.");
        }
    }

    analysis_report_str
}

#[wasm_bindgen]
pub fn analyze_tcx(s: &str) -> String {
    let mut data = BufReader::new(s.as_bytes());
    let res = tcx::read(&mut data);
    let mut analyzer = location_analyzer::LocationAnalyzer::new();
    let mut cadence_analyzer = cadence_analyzer::CadenceAnalyzer::new();
    let mut power_analyzer = power_analyzer::PowerAnalyzer::new();
    let activities = res.activities.unwrap();

    for activity in activities.activities {
        for lap in activity.laps {
            for track in lap.tracks {
                for trackpoint in track.trackpoints {
                    let time = trackpoint.time.timestamp() * 1000 + trackpoint.time.timestamp_subsec_millis() as i64;
                    let position = trackpoint.position.unwrap();
                    let altitude = trackpoint.altitude_meters.unwrap();

                    analyzer.append_location(time as u64, position.latitude, position.longitude, altitude);
                    analyzer.update_speeds();

                    // Get the cadence reading.
                    let cadence = trackpoint.cadence;
                    match cadence {
                        None => {
                        }
                        Some(cadence) => {
                            cadence_analyzer.append_sensor_value(time as u64, cadence as f64);
                        }
                    }

                    // Get the extensions.
                    let extensions = trackpoint.extensions.as_ref();
                    match extensions {
                        None => {
                        }
                        Some(extensions) => {
                            // Get the power reading.
                            let tpx = extensions.tpx.as_ref();
                            match tpx {
                                None => {
                                }
                                Some(tpx) => {
                                    let watts = tpx.watts;
                                    match watts {
                                        None => {
                                        }
                                        Some(watts) => {
                                            power_analyzer.append_sensor_value(time as u64, watts as f64);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // For calculations that only make sense once all the points have been added.
    analyzer.analyze();

    // Copy items to the final report.
    let analysis_report_str = make_final_report(&analyzer, Some(&power_analyzer), Some(&cadence_analyzer));

    analysis_report_str
}
