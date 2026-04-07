use reqwest;

fn main() {
    let _client = reqwest::Client::builder()
        .unix_socket("/tmp/foo.sock");
}
