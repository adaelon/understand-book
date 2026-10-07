fn main() {
    if server::presentation_worker().is_err() {
        std::process::exit(1);
    }
}
