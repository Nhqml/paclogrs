use clap::Parser;

#[derive(Parser, Debug)]
#[clap(name = "paclogrs", version)]
#[clap(about = "Pacman log but prettier", long_about = None)]
pub struct Cli {
    #[clap(help = "Packages to list (supports *-glob)")]
    pub packages: Vec<String>,

    #[clap(long, help = "Filter changes before this date (included)")]
    pub before: Option<String>,

    #[clap(long, help = "Filter changes after this date (included)")]
    pub after: Option<String>,
}
