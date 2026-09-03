fn main() {
    match conduit_tongues::run_research_json() {
        Ok(report) => println!("{report}"),
        Err(error) => {
            eprintln!("Tongues research research experiment refused: {error:?}");
            std::process::exit(1);
        }
    }
}
