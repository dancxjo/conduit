use super::*;

fn dispatch(arguments: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::try_parse_from(arguments).expect("Pico command parses");
    let Command::Make(args) = cli.command else {
        panic!("expected a make command");
    };
    run_make(args.target, &cli.global)
}

#[test]
fn explicit_bootsel_uses_maintenance_dispatch_for_both_entrances() {
    for entrance in ["pico", "pico-local"] {
        // A local workflow refuses remote images before doctor/build/flash.
        // BOOTSEL dry-run returns before discovery, Cargo, USB, or flashing.
        // Thus this also fails safely if the alias regresses to run_local.
        for image in ["--usb-remote", "--pete-capstone", "--appliance-hello"] {
            dispatch(&["xtask", "--dry-run", "make", entrance, "bootsel", image])
                .unwrap_or_else(|error| panic!("{entrance} bootsel {image}: {error}"));
        }
    }
}

#[test]
fn missing_or_explicit_local_keeps_the_local_workflow_guard() {
    for entrance in ["pico", "pico-local"] {
        for subcommand in [None, Some("local")] {
            let mut arguments = vec!["xtask", "--dry-run", "make", entrance];
            arguments.extend(subcommand);
            arguments.push("--usb-remote");
            let error = dispatch(&arguments).expect_err("local workflow refuses remote images");
            assert!(error.to_string().contains("complete `pico local` workflow"));
        }
    }
}
