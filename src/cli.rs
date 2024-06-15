use clap::Parser;

#[derive(Parser)]
#[command(name = crate_name!())]
#[command(version = crate_version!())]
#[command(about = "preboot-oxide: PXE Boot Server utility\nProject home: https://github.com/alexculea/Preboot-Oxide", long_about = None)]
pub struct Cli {
}
