use std::collections::HashMap;
use std::{io, fs};
use std::time::{Instant, Duration};

use serde::{Serialize, Deserialize};
use hyper::{body::HttpBody as _};
use hyper_tls::HttpsConnector;
use strum_macros::EnumIter;
use strum::IntoEnumIterator;
use colored::*;

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
}

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    api_address: String,
    verbose: Option<bool>,
    tests: Vec<Endpoint>,
    bearer_token: Option<String>,
    time_boundaries: Option<[u128; 3]>, // (green), yellow, red, timeout
}

// Checks if a given method matched one of HttpMethod
fn validate_http_method(method: &String) -> Option<HttpMethod> {
    return HttpMethod::iter().find(|http_method|
         http_method.to_string() == method.to_string().to_lowercase());
}

async fn fetch_url(url: hyper::Uri, method: HttpMethod, verbose: bool, timeout: u128,
     body: String, response_time: &mut u128 /*OUT*/) -> Result<hyper::Response<hyper::Body>> {
     
    // TLS implementation to enable https requests
    let https = HttpsConnector::new();

    // map local http methods to the ones used by hyper
    let req = hyper::Request::builder()
        .method(match method {
            HttpMethod::get => hyper::Method::GET,
            HttpMethod::post => hyper::Method::POST,
            HttpMethod::put => hyper::Method::PUT,
            HttpMethod::patch => hyper::Method::PATCH,
            HttpMethod::delete => hyper::Method::DELETE,
            HttpMethod::head => hyper::Method::HEAD,
            HttpMethod::options => hyper::Method::OPTIONS,
        })
        .uri(url)
        .header("content-type", "application/json")
        .body(hyper::Body::from(body))?;

    let client = hyper::Client::builder().build(https);

    let now = Instant::now();

    let future_response = client.request(req);

    let mut response: hyper:: Response<hyper::Body>;

    response = match tokio::time::timeout(Duration::from_millis(timeout.try_into().unwrap()),
     future_response).await {
        Ok(result) => match result {
            Ok(res) => res,
            Err(e) => return Err(Box::new(e)),
        },
        Err(_) => return Err("Request timed out.".into()),
    };

    *response_time = now.elapsed().as_millis();

    println!("Response Status: {}", response.status());

    if verbose {
        println!("Response Header: {:#?}", response.headers());

        println!("Response Body: "); 
        while let Some(next) = response.data().await {
            let chunk = next?;
            io::Write::write_all(&mut io::stdout(), &chunk)?;
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

    // Get verbose value, default to true
    let verbose = rest_test_config.verbose.unwrap_or(true);

    let test_count = rest_test_config.tests.len();
    let mut method: HttpMethod;
    let mut route: &String;
    let mut test_index = 0;
    let mut tests_passed = 0;
    let mut response_time: u128 = 0;

    // Get boundaries, set to default values if not found
    let time_boundaries = rest_test_config.time_boundaries.unwrap_or([500, 1000, 10000]);

    for test in rest_test_config.tests.iter() { 
        test_index += 1;

        // Determine criticalness, default to false
        let is_critical = test.critical.unwrap_or(false);

        // Print current test index
        println!("Test {}/{}", test_index, test_count);

        // Print test description if available
        match &test.it { 
            Some(description) => println!("it {}", description),
            None => (),
        };

        // Check if the http method is valid
        method = match validate_http_method(&test.method) {
            Some(value) => value,
            None => panic!("Unknown or unsupported method {}", test.method),
        };

        // Construct the api url
        route = &test.route;
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

        // Send the request and get the response
        let response = match fetch_url(url, method,
             verbose, time_boundaries[2], body, &mut response_time).await {
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

        let response_time_output = format!("Response time: {} ms", response_time);

        if response_time < time_boundaries[0] {
            println!("{}", response_time_output.green());
        } else if response_time < time_boundaries[1] {
            println!("{}", response_time_output.yellow());
        } else {
            println!("{}", response_time_output.red());
        }
 
        // Check expectations
        println!("Expected Status: {}", response.status());

        // Print outcome
        if response.status() == test.status {
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
