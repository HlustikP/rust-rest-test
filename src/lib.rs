use std::{io, fs};

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
}

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    api_address: String,
    verbose: Option<bool>,
    tests: Vec<Endpoint>,
    bearer_token: Option<String>,
    username: Option<String>,
    password: Option<String>,
    time_boundaries: [u32; 3],
}

fn validate_http_method(method: &String) -> Option<HttpMethod> {
    for http_method in HttpMethod::iter() {
        if http_method.to_string() == method.to_string().to_lowercase() {
            return Some(http_method);
        }
    }
    return None;
}

async fn fetch_url(url: hyper::Uri, method: HttpMethod, verbose: bool) -> 
    Result<hyper::Response<hyper::Body>> {
    
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
        .body(hyper::Body::from(r#"{"library":"hyper"}"#))?;

    let client = hyper::Client::builder().build(https);

    let mut response = client.request(req).await?;

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

    let verbose = match rest_test_config.verbose {
        None => true, // Default to verbose output
        Some(value) => value,
    };

    let test_count = rest_test_config.tests.len();
    let mut method: HttpMethod;
    let mut route: &String;
    let mut test_index = 0;
    let mut tests_passed = 0;

    for test in rest_test_config.tests.iter() { 
        test_index += 1;

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
            None => panic!("Unknown or unsupported method {}", test.method.to_string()),
        };

        // Construct the api url
        route = &test.route;
        let url = api_address.to_owned() + route;
        let url = url.parse::<hyper::Uri>().unwrap();
    
        // Send the request and get the response
        let response = match fetch_url(url, method, verbose).await {
            Ok(res) => res,
            Err(error) => panic!("Error while sending request: {:?}",
                error),
        };

        // Check expectations
        println!("Expected Status: {}", response.status());

        // Print outcome
        if response.status() == test.status {
            tests_passed += 1;
            println!("{}", "TEST PASSED\n".green().bold())
        } else {
            println!("{}", "TEST FAILED\n".red().bold())
        }
    }

    println!("{} out of {} tests passed.", tests_passed, test_count)
}
