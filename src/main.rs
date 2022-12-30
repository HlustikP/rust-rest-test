use rust_rest_test as rrt;

#[tokio::main]
async fn main() {
    rrt::execute_tests().await;
}
