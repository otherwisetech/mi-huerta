fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match mi_huerta_cli::run(&args) {
        Some(code) => std::process::exit(code),
        None => println!("{}", mi_huerta_cli::usage()),
    }
}
