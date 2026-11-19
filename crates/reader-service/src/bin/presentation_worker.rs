fn main() {
    if reader_service::presentation_worker().is_err() {
        std::process::exit(1);
    }
}
