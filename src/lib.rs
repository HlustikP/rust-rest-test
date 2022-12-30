use std::{env, path::PathBuf, io, fs};

use serde::{Serialize, Deserialize};
use hyper::{body::HttpBody as _, StatusCode};
use hyper_tls::HttpsConnector;
use strum_macros::EnumIter;
use strum::IntoEnumIterator;
use colored::*;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[allow(non_camel_case_types)]
#[derive(strum_macros::Display, EnumIter)]
enum HttpMethods {
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
}

fn get_cwd() -> PathBuf {
    return env::current_dir().unwrap();
}

fn validate_http_method(method: &String) -> bool {
    for http_method in HttpMethods::iter() {
        if http_method.to_string() == method.to_string().to_lowercase() {
            return true;
        }
    }
    return false
}

async fn fetch_url(url: hyper::Uri, verbose: bool) -> Result<StatusCode> {
    let https = HttpsConnector::new();

    let client = hyper::Client::builder().
        build::<_, hyper::Body>(https);

    let mut response = client.get(url).await?;

    println!("Response Status: {}", response.status());

    if verbose {
        println!("Response Header: {:#?}", response.headers());

        println!("Response Body: "); 
        while let Some(next) = response.data().await {
            let chunk = next?;
            io::Write::write_all(&mut io::stdout(), &chunk)?;
        }
    };

    return Ok(response.status());
}

pub async fn execute_tests() {
    let cwd = get_cwd();
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
    let mut method: &String;
    let mut route: &String;
    let mut i = 1;

    for test in rest_test_config.tests.iter() {
        println!("Test {}/{}", i, test_count);
        method = &test.method;
        route = &test.route;

        if !validate_http_method(method) {
            panic!("Invalid http method used: {:?}", method);
        }
    
        let url = api_address.to_owned() + route;
    
        let url = url.parse::<hyper::Uri>().unwrap();
    
        let result = match fetch_url(url, verbose).await {
            Ok(res) => res,
            Err(error) => panic!("Error while sending request: {:?}",
                error),
        };

        println!("Expected Status: {}", result);

        if result == test.status {
            println!("{}", "TEST PASSED\n".green().bold())
        } else {
            println!("{}", "TEST FAILED\n".red().bold())
        }

        i += 1;
    }
}
