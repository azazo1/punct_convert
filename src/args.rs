use clap::Parser;

#[derive(Parser)]
#[command(author, about, version, long_about = None)]
pub struct AppArgs {
    #[clap(short, long, help = "convert current clipboard and quit.")]
    pub oneshot: bool,
    #[clap(
        short = 'n',
        long,
        help = "inspect current clipboard and print planned conversions without modifying clipboard."
    )]
    pub dry_run: bool,
}
