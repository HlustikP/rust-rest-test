use std::collections::HashMap;
use std::{io, fs};
use std::time::{Instant, Duration};

use hyper::http::HeaderValue;
use serde::{Serialize, Deserialize};
use hyper::{body::HttpBody as _};
use hyper_tls::HttpsConnector;
use strum_macros::EnumIter;
use strum::IntoEnumIterator;
use colored::*;
use bytes::BufMut;

mod utils;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

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
}

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    api_address: String,
    verbose: Option<bool>,
    tests: Vec<Endpoint>,
    time_boundaries: Option<[u128; 3]>, // (green), yellow, red, timeout
    caption_path: Option<Vec<String>>,
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

// Checks if a given method matched one of HttpMethod
fn validate_http_method(method: &String) -> Option<HttpMethod> {
    return HttpMethod::iter().find(|http_method|
         http_method.to_string() == method.to_string().to_lowercase());
}

async fn fetch_url(test_request: &mut TestRequest<'_>)
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
            println!("Token: {}", token);
            composed_token += &token.clone();

            match req_builder.headers_mut() {
                Some(map) => {
                    println!("Composed token: {}", composed_token);
                    map.insert("Authorization", composed_token.parse::<HeaderValue>()?);
                },
                None => (),
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

    if test_request.verbose {
        println!("Response Header: {:#?}", response.headers());

        if !test_request.buffer.is_empty() {
            println!("Response Body: ");
            io::Write::write_all(&mut io::stdout(), test_request.buffer)?;
        }
    };

    return Ok(response);
}

pub async fn execute_tests() {
    let cwd = utils::get_cwd();
    let rest_test_filename = "rest-test.yaml";

    let test_config_file = fs::File::open(cwd.join(rest_test_filename)).
        expect("Could not open file.");
    
    let rest_test_config: Config = serde_yaml::from_reader(test_config_file).
        expect("Could not read values.");

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
            Some(description) => println!("{}", description),
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

        println!("Capture Key: {}", test.bearer_token.clone().unwrap_or("".to_string()));

        // Construct request data struct
        let mut test_request = TestRequest {
            url: &url,
            method: &method,
            verbose,
            timeout: time_boundaries[2],
            body,
            response_time: &mut response_time,
            buffer: &mut buffer,
            // bearer_token: match captures.get(&"bearer_token".to_string()) {
            //          Some(val) => Some(val.clone()),
            //          None => None,
            // },
            bearer_token: match captures.get(&test.bearer_token.clone().unwrap_or("".to_string())) {
                Some(val) => Some(val.clone()),
                None => None,
            },
        };

        // Send the request and get the response
        let response = match fetch_url(&mut test_request).await {
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

    println!("{} out of {} tests passed.", tests_passed, test_count)
}
