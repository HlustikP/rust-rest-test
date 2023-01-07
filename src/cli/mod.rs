#[derive(clap::Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
   /// Path to the config file, will look for it in the executable's cwd per default
   #[arg(short, long)]
   pub file: Option<String>,
}
