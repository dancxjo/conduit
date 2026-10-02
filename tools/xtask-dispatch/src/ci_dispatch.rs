mod pages_resolver;
mod pipeline;

pub(super) fn run(arguments: &[String]) -> Result<(), String> {
    match arguments.get(1).map(String::as_str) {
        Some("pipeline") => pipeline::run(&arguments[2..]),
        Some("rust-toolchain-preflight") => rust_toolchain_preflight(arguments),
        Some("standalone-locks") => standalone_locks(arguments),
        Some("storage-report") => crate::local_storage::run(arguments),
        Some("storage-reclaim") => crate::local_storage::reclaim::run(arguments),
        Some("pages-resolver-proof") => pages_resolver::run(arguments),
        Some(command) => Err(format!("unsupported ci command: {command}")),
        None => Err("missing ci command".to_owned()),
    }
}

fn rust_toolchain_preflight(arguments: &[String]) -> Result<(), String> {
    for argument in arguments.iter().skip(2) {
        if argument != "--locked" {
            return Err(format!(
                "unsupported ci rust-toolchain-preflight argument: {argument}"
            ));
        }
    }
    let root =
        std::env::current_dir().map_err(|error| format!("resolve repository root: {error}"))?;
    crate::rust_toolchain::run(&root)
}

fn standalone_locks(arguments: &[String]) -> Result<(), String> {
    for argument in arguments.iter().skip(2) {
        if argument != "--locked" {
            return Err(format!(
                "unsupported ci standalone-locks argument: {argument}"
            ));
        }
    }
    crate::standalone_locks::run().map_err(|error| error.to_string())
}
