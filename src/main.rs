use rust_rest_test as rrt;

#[tokio::main]
async fn main() {
    let test_file = rrt::get_config_file();

    rrt::execute_tests(test_file).await;
}
