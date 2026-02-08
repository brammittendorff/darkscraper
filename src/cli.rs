use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "darkscraper", about = "Dark web crawler & entity extractor")]
pub struct Cli {
    /// Path to config file
    #[arg(short, long, default_value = "config/default.toml")]
    pub config: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start crawling from seed URLs
    Crawl {
        /// Seed URLs (comma-separated or file path)
        #[arg(short, long)]
        seeds: Option<String>,

        /// Single seed URL
        #[arg(long)]
        seed: Option<String>,

        /// Maximum crawl depth
        #[arg(short, long)]
        depth: Option<u32>,
    },
    /// Search crawled data
    Search {
        /// Full-text query
        #[arg(short, long)]
        query: Option<String>,

        /// Search by entity value
        #[arg(short, long)]
        entity: Option<String>,

        /// Entity type filter (email, bitcoin, phone, etc.)
        #[arg(short = 't', long)]
        entity_type: Option<String>,

        /// Max results
        #[arg(short, long, default_value = "20")]
        limit: i64,
    },
    /// Show crawl status and database stats
    Status,
    /// Export crawled data
    Export {
        /// Output format (json)
        #[arg(short, long, default_value = "json")]
        format: String,

        /// Output file
        #[arg(short, long)]
        output: String,
    },
}
