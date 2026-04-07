fn main() {
    let _builder = reqwest::Client::builder().unix_socket("/tmp/socket.sock");
}
