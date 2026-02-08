use anyhow::Result;
use darkscraper_core::config::AppConfig;
use darkscraper_search::SearchEngine;
use darkscraper_storage::Storage;

pub async fn run_search(
    config: AppConfig,
    query: Option<String>,
    entity: Option<String>,
    entity_type: Option<String>,
    limit: i64,
) -> Result<()> {
    let storage = Storage::new(&config.database.postgres_url).await?;
    let search = SearchEngine::new(storage.pool().clone());

    if let Some(q) = query {
        let results = search.search_text(&q, limit).await?;
        println!("Found {} results:\n", results.len());
        for r in results {
            println!(
                "  [{}] {} - {}",
                r.network,
                r.url,
                r.title.unwrap_or_default()
            );
            if let Some(snippet) = r.snippet {
                println!("    {}", &snippet[..snippet.len().min(100)]);
            }
            println!();
        }
    } else if let Some(entity_val) = entity {
        let results = search
            .search_entity(entity_type.as_deref(), &entity_val, limit)
            .await?;
        println!("Found {} entity matches:\n", results.len());
        for r in results {
            println!(
                "  [{}] {} = {} (page: {})",
                r.entity_type,
                r.value,
                r.page_url,
                r.page_title.unwrap_or_default()
            );
        }
    } else {
        println!("Provide --query or --entity to search");
    }

    Ok(())
}

pub async fn run_status(config: AppConfig) -> Result<()> {
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

    println!("Pages crawled:    {}", pages);
    println!("Entities found:   {}", entities);
    println!("Links discovered: {}", links);
    println!("Correlations:     {}", correlations);
    println!("Dead URLs:        {}", dead);

    Ok(())
}

pub async fn run_export(config: AppConfig, format: &str, output: &str) -> Result<()> {
    let storage = Storage::new(&config.database.postgres_url).await?;

    match format {
        "json" => {
            let pages = storage.get_page_count().await?;
            println!("Exporting {} pages to {}", pages, output);
            println!("Export not yet fully implemented (Phase 2)");
        }
        _ => {
            println!("Unsupported format: {}. Use 'json'.", format);
        }
    }

    Ok(())
}
