fn main() {
    match conduit_tongues::run_dynamics_analysis_json() {
        Ok(report) => println!("{report}"),
        Err(error) => {
            eprintln!("Tongues dynamics analysis refused: {error:?}");
            std::process::exit(1);
        }
    }
}
