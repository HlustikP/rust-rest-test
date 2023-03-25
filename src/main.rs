use rrt::{ get_config_file, execute_tests };

#[tokio::main]
async fn main() {
    let test_file = get_config_file();

    execute_tests(test_file).await;
}
