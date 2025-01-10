use clap::Parser;

#[derive(Parser)]
#[command(name = crate_name!())]
#[command(version = crate_version!())]
#[command(about = "preboot-oxide: PXE Boot Server utility\nProject home: https://github.com/alexculea/Preboot-Oxide", long_about = None)]
pub struct Cli {
    /// Sets the output verbosity level. Available levels: error, warn, info, debug, trace. Example: -v, -vv, -vvv
    #[arg(short, action = clap::ArgAction::Count)]
    verbosity: Option<u8>,
}

pub fn parse() -> Option<String> {
    let args = Cli::parse();

    const LEVELS: [&str; 5] = ["error", "warn", "info", "debug", "trace"];
    LEVELS
        .get(args.verbosity.unwrap_or(0) as usize)
        .map(|s| s.to_string())
}
