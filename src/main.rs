#[tokio::main]
async fn main() {
    let test_file = rrtlib::get_config_file();

    rrtlib::execute_tests(test_file).await;
}
