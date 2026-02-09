use anyhow::Result;
use darkscraper_core::config::AppConfig;
use darkscraper_search::SearchEngine;
use darkscraper_storage::Storage;

pub async fn run(
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
