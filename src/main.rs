mod cli;

fn main() {
    let exit_code = cli::run();
    std::process::exit(exit_code);
}
