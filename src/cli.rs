use clap::Parser;

#[derive(Parser, Debug)]
#[clap(name = "paclogrs", version)]
#[clap(about = "Pacman log but prettier", long_about = None)]
pub struct Cli {
    #[clap(help = "Packages to list (supports *-glob)")]
    pub packages: Vec<String>,
}
