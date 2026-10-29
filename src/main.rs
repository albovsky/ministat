fn main() {
    std::process::exit(ministat::cli::run(
        &std::env::args().skip(1).collect::<Vec<_>>(),
    ));
}
