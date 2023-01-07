use std::collections::HashMap;
use std::path::PathBuf;
use std::{io, io::Write, fs, path};
use std::time::{Instant, Duration};

use hyper::http::HeaderValue;
use serde::{Serialize, Deserialize};
use hyper::{body::HttpBody as _};
use hyper_tls::HttpsConnector;
use strum_macros::EnumIter;
use strum::IntoEnumIterator;
use colored::*;
use bytes::BufMut;
use clap::Parser;
use chrono::{self, Datelike};

mod utils;
mod cli;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

// Prints a string and appens a newline if a given condition evaluates to true
macro_rules! printlnif {
    ($condition: expr, $($x:tt)*) => { 
        if $condition { 
            println!($($x)*);
        }
    }
}

#[allow(non_camel_case_types)]
#[derive(strum_macros::Display, EnumIter)]
enum HttpMethod {
    get,
    post,
    put,
    patch,
    delete,
    options,
    head,
}

#[derive(Debug, Serialize, Deserialize)]
struct Endpoint {
    it: Option<String>,
    critical: Option<bool>,
    route: String,
    method: String,
    status: u16,
    json_body: Option<HashMap<String, String>>,
    time_boundaries: Option<[u128; 3]>, // (green), yellow, red, timeout
    capture: Option<HashMap<String, String>>,
    bearer_token: Option<String>,
    auto_description: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    api_address: String,
    verbose: Option<bool>,
    tests: Vec<Endpoint>,
    time_boundaries: Option<[u128; 3]>, // (green), yellow, red, timeout
    caption_path: Option<Vec<String>>,
    to_file: Option<PathBuf>,
}

struct TestRequest<'a> {
    url: &'a hyper::Uri,
    method: &'a HttpMethod,
    verbose: bool,
    timeout: u128,
    body: String,
    response_time: &'a mut u128,
    buffer: &'a mut bytes::BytesMut,
    bearer_token: Option<String>,
}

// Get the number of files matching a certain pattern inside a directory
fn get_file_iteration(directory: path::PathBuf, pattern: &String) -> Result<usize> {
    // Get all files in directory matching the pattern
    return Ok(fs::read_dir(directory)?
        .into_iter()
        .filter(|file| file.is_ok())
        .map(|file| file.unwrap().path()) // safe unwrap call inside Ok
        .filter(|file| file.is_file())
        .filter(|file| file.to_str().unwrap_or_default().contains(pattern))
        .count());
}

// Creates the name of the logfile based on the current time
fn construct_logfile_name(directory: PathBuf) -> Result<String> {
    // rrt-YEAR-MONTH-DAY-ITERATOR.log
    // rrt-2023-07-01-00.log
    // rrt-2023-07-01-01.log etc
    let current_date = chrono::Utc::now().date_naive();

    let date_filename = format!(
        "rrt-{}-{}-{}",
        current_date.year() - 2000,
        current_date.month0(),
        current_date.day0());

    let iterations = get_file_iteration(directory, &date_filename)?;

    // Prepend 0 on single digit itertions counts
    let iteration_string = if utils::get_num_digits(iterations, 10usize) < 10 {
        "0".to_string() + &iterations.to_string()
    } else {
        iterations.to_string()
    };

    return Ok(date_filename + &iteration_string);
}

// Handler for post-tests logfile creation
fn write_logfile(log_buffer: Option<String>, directory: PathBuf) {
    if log_buffer.is_some() {
        let filename = construct_logfile_name(directory);
        let filename_ref;

        let file_path = path::Path::new(match filename{
            Ok(name) => {
                filename_ref = name;
                &filename_ref
            },
            Err(error) => {
                println!("Error while retrieving path to logfile: {}", error);
                return;
            }
        });

        let display = file_path.display();

        let buffer = match log_buffer {
            Some(buff) => buff,
            None => "Log Buffer got corrupted.".to_string(),
        };

        // Open a file in write-only mode, creates file if nonexistant
        let mut file = match fs::File::create(file_path) {
            Err(why) => panic!("couldn't create {}: {}", display, why),
            Ok(file) => file,
        };

        // Write log buffer into the file
        match file.write_all(buffer.as_bytes()) {
            Err(why) => panic!("couldn't write to {}: {}", display, why),
            Ok(_) => println!("successfully wrote to {}", display),
        }
    }
}

// Logging handler, prints formatted_string if print_condition is true
fn log(formatted_string: String, print_condition: Option<bool>, log_buffer: &mut Option<String> /*IN-OUT*/) {
    if let Some(condition) = print_condition {
        printlnif!(condition, "{}", formatted_string);
        if let Some(buffer) = log_buffer { 
            *buffer += &formatted_string;
        };
    };
}

// Reads in the config file
pub fn get_config_file() -> path::PathBuf {
    let args = cli::Args::parse();

    // Use command line input
    if let Some(config_path) = args.file.as_deref() {
        return path::PathBuf::from(config_path);
    } else {
        // Otherwise use default (looking for it inside the cwd)
        let cwd = utils::get_cwd();
        let rest_test_filename = "rest-test.yaml";

        return cwd.join(rest_test_filename);
    }
}

// Checks if a given method matched one of HttpMethod
fn validate_http_method(method: &String) -> Option<HttpMethod> {
    return HttpMethod::iter().find(|http_method|
         http_method.to_string() == method.to_string().to_lowercase());
}

async fn fetch_url(test_request: &mut TestRequest<'_>, log_buffer: &mut Option<String> /*IN-OUT*/)
     -> Result<hyper::Response<hyper::Body>> {
     
    // TLS implementation to enable https requests
    let https = HttpsConnector::new();

    // map local http methods to the ones used by hyper
    let mut req_builder = hyper::Request::builder()
        .method(match test_request.method {
            HttpMethod::get => hyper::Method::GET,
            HttpMethod::post => hyper::Method::POST,
            HttpMethod::put => hyper::Method::PUT,
            HttpMethod::patch => hyper::Method::PATCH,
            HttpMethod::delete => hyper::Method::DELETE,
            HttpMethod::head => hyper::Method::HEAD,
            HttpMethod::options => hyper::Method::OPTIONS,
        })
        .uri(test_request.url);

    match &test_request.bearer_token {
        Some(token) => {
            let mut composed_token = String::from("");
            composed_token += "Bearer ";
            log(format!("Token: {}", token), Some(true), log_buffer);
            composed_token += &token.clone();

            if let Some(map) = req_builder.headers_mut() {
                println!("Composed token: {}", composed_token);
                map.insert("Authorization", composed_token.parse::<HeaderValue>()?);
            };
        },
        None => (),
    };

    let req = req_builder.body(hyper::Body::from(test_request.body.clone()))?;

    let client = hyper::Client::builder().build(https);

    let now = Instant::now();

    let future_response = client.request(req);

    let mut response: hyper:: Response<hyper::Body>;

    response = match tokio::time::timeout(Duration::from_millis(test_request.timeout.try_into().unwrap()),
     future_response).await {
        Ok(result) => match result {
            Ok(res) => res,
            Err(e) => return Err(Box::new(e)),
        },
        Err(_) => return Err("Request timed out.".into()),
    };

    *test_request.response_time = now.elapsed().as_millis();

    println!("Response Status: {}", response.status());

    // stream body data into buffer
    while let Some(next) = response.data().await {
        test_request.buffer.put(next?);
    }

    printlnif!(test_request.verbose, "Response Header: {:#?}", response.headers());

    if !test_request.buffer.is_empty() {
        printlnif!(test_request.verbose, "Response Body: ");
        io::Write::write_all(&mut io::stdout(), test_request.buffer)?;
    }

    return Ok(response);
}

pub async fn execute_tests(config_file: path::PathBuf) {
    // Open and read config file
    let test_config_file = match fs::File::open(config_file) {
        Ok(file) => file,
        Err(error) => {
            println!("Error while trying to open config file: {}", error);
            return;
        }
    };
    
    // Parse config yaml file
    let rest_test_config: Config = match serde_yaml::from_reader(test_config_file) {
        Ok(config) => config,
        Err(error) => {
            println!("Error while parsing config file: {}", error);
            return;
        }
    };

    // Set buffer to Some if a destination directory is specified
    let mut log_buffer: Option<String> = None;
    if rest_test_config.to_file.is_some() { 
        log_buffer = Some(Default::default());
    };

    let api_address = &rest_test_config.api_address;

    // Get verbose value, default to false
    let verbose = rest_test_config.verbose.unwrap_or(false);

    let test_count = rest_test_config.tests.len();
    let mut test_index = 0;
    let mut tests_passed = 0;

    // Get boundaries, set to default values if not found
    let mut time_boundaries = rest_test_config.time_boundaries.unwrap_or([500, 1000, 10000]);

    let mut captures: HashMap<String, String> = Default::default();

    for test in rest_test_config.tests.iter() {
        let mut response_time: u128 = 0;
        test_index += 1;

        // Determine criticalness, default to false
        let is_critical = test.critical.unwrap_or(false);

        // Overwrite time boundaries if there is a local definition
        time_boundaries = match test.time_boundaries {
            Some(value) => value,
            None => time_boundaries,
        };

        // Print current test index
        println!("Test {}/{}", test_index, test_count);

        // Print test description if available
        match &test.it { 
            Some(description) => log(description.clone(),
             Some(true), &mut log_buffer),
            None => (),
        };

        // Check if the http method is valid
        let method = match validate_http_method(&test.method) {
            Some(value) => value,
            None => panic!("Unknown or unsupported method {}", test.method),
        };

        // Construct the api url
        let route = &test.route;
        let url = api_address.to_owned() + route;
        let url = url.parse::<hyper::Uri>().unwrap();

        // Construct json body for the request
        let mut body: String = String::from("{");
        match &test.json_body{
            Some(value_map) => for (key, val) in value_map.iter() {
                    body += &format!("\"{}\":\"{}\",", key, val);
                },
            None => body = String::from(""),
        };

        // json doesnt allow a comma after the last key-value pair
        if body.ends_with(',') {
            body.pop();
        }
        body += "}";

        // Create buffer for the response body
        let mut buffer = bytes::BytesMut::with_capacity(512);

        printlnif!(verbose, "Capture Key: {}", test.bearer_token.clone().unwrap_or_default());

        // Construct request data struct
        let mut test_request = TestRequest {
            url: &url,
            method: &method,
            verbose,
            timeout: time_boundaries[2],
            body,
            response_time: &mut response_time,
            buffer: &mut buffer,
            bearer_token: captures.get(&test.bearer_token.clone().unwrap_or_default()).cloned(),
        };

        // Send the request and get the response
        let response = match fetch_url(&mut test_request, &mut log_buffer).await {
            Ok(res) => res,
            Err(error) => { 
                println!("Error while sending request: {}", error);
                if is_critical {
                    println!("Test marked as 'critical' failed, cancelling all further tests.");
                    return;
                }
                continue
            },
        };

        let response_status = response.status();

        // Parse the response body as long as its not empty and (probably) a json
        if !buffer.is_empty() && buffer[0] == b'{' {
        let json_body: serde_json::Value = match serde_json::from_str(&String::from_utf8_lossy(&buffer)) {
                Ok(value) => value,
                Err(error) => {
                    println!("Error while parsing response body as json: {}\n", error);
                    continue;
                },
            };
    
            // Capture desired values from the response body
            match &test.capture {
                Some(capture) => {
                    for (key, value) in capture.iter() {
                        let captured_value = &json_body["data"][value];
                        if !captured_value.is_null() {
                            let mut string_captured = json_body["data"][value].to_string();

                            // Remove Double Quotes
                            string_captured.pop();
                            if !string_captured.is_empty() {
                                string_captured.remove(0);
                            }

                            captures.insert(key.to_string(), string_captured);
                        } else {
                            println!("Cannot capture nonexistent value '{}'", value.bold());
                        }
                    }
                },
                None => (),
            }
        }

        let response_time_output = format!("Response time: {} ms", response_time);

        if response_time < time_boundaries[0] {
            println!("{}", response_time_output.green());
        } else if response_time < time_boundaries[1] {
            println!("{}", response_time_output.yellow());
        } else {
            println!("{}", response_time_output.red());
        }
 
        // Check expectations
        println!("Expected Status: {}", test.status);

        // Print outcome
        if response_status == test.status {
            tests_passed += 1;
            println!("{}", "TEST PASSED\n".green().bold())
        } else {
            if is_critical {
                println!("Test marked as 'critical' failed, cancelling all further tests.");
                return;
            }

            println!("{}", "TEST FAILED\n".red().bold())
        }
    }

    println!("{} out of {} tests passed.", tests_passed, test_count);

    if let Some(directory) = rest_test_config.to_file { 
        write_logfile(log_buffer, directory);
    };
}
