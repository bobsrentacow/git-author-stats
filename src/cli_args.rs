use clap::Parser;

//----
// Command Line Parsing

#[derive(Debug, Parser)]
#[command(
    name = "git-author-stats",
    author = "Matt Mahin",
    about = "tool to track how much code is being authored by each developer",
)]
pub struct Args {
    /// Sort alphabetically by author name, instead of by number of lines
    #[arg(short, long)]
    pub alphabetical: bool,

    /// Display counts as percentages
    #[arg(short = 'p', long = "percent")]
    pub as_percent: bool,

    /// Show excluded files
    #[arg(long = "show-excluded")]
    pub show_excluded: bool,

    /// branch to analyze
    #[arg(short, long)]
    pub branch: Option<String>,

    /// Only look at commits before this date: YYYY-MM-DD.  Defaults to all commits
    #[arg(short, long)]
    pub date: Option<String>,

    /// Path of folder within the git repo to analyze
    #[arg(index = 1, default_value = ".")]
    pub path: String,
}
