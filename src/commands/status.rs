use anyhow::Result;
use darkscraper_core::config::AppConfig;
use darkscraper_storage::Storage;

pub async fn run(config: AppConfig) -> Result<()> {
    let storage = Storage::new(&config.database.postgres_url).await?;

    match storage.check_connectivity().await {
        Ok(()) => println!("Database: connected"),
        Err(e) => {
            println!("Database: ERROR - {}", e);
            return Ok(());
        }
    }

    storage.run_migrations().await?;

    let pages = storage.get_page_count().await?;
    let entities = storage.get_entity_count().await?;
    let links = storage.get_link_count().await?;
    let correlations = storage.get_correlation_count().await?;
    let dead = storage.get_dead_url_count().await?;

    println!("\n╔══════════════════════════════════════════════╗");
    println!("║           DarkScraper Status                 ║");
    println!("╠══════════════════════════════════════════════╣");
    println!("║ Pages crawled:      {:>20}    ║", pages);
    println!("║ Entities found:     {:>20}    ║", entities);
    println!("║ Links discovered:   {:>20}    ║", links);
    println!("║ Correlations:       {:>20}    ║", correlations);
    println!("║ Dead URLs:          {:>20}    ║", dead);
    println!("╚══════════════════════════════════════════════╝\n");

    Ok(())
}
