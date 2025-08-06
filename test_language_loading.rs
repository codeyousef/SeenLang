// Test program to verify language file loading works
use seen_std::toml::{LanguageDefinition, LanguageLoader};

fn main() {
    println!("Testing language file loading...");
    
    // Test loading English language file
    match LanguageDefinition::from_file("languages/en.toml") {
        Ok(definition) => {
            println!("Successfully loaded English language definition:");
            println!("  Name: {}", definition.name);
            println!("  Keywords: {} total", definition.keywords().len());
            println!("  Performance stats: {:?}", definition.performance_stats());
            
            // Test some keyword lookups
            println!("  'func' -> {:?}", definition.lookup_keyword("func"));
            println!("  'if' -> {:?}", definition.lookup_keyword("if"));
            println!("  'nonexistent' -> {:?}", definition.lookup_keyword("nonexistent"));
        }
        Err(e) => {
            println!("Failed to load English definition: {}", e);
        }
    }
    
    // Test loading Arabic language file  
    match LanguageDefinition::from_file("languages/ar.toml") {
        Ok(definition) => {
            println!("\nSuccessfully loaded Arabic language definition:");
            println!("  Name: {}", definition.name);
            println!("  Keywords: {} total", definition.keywords().len());
            println!("  Performance stats: {:?}", definition.performance_stats());
            
            // Test some keyword lookups
            println!("  'دالة' -> {:?}", definition.lookup_keyword("دالة"));
            println!("  'إذا' -> {:?}", definition.lookup_keyword("إذا"));
        }
        Err(e) => {
            println!("Failed to load Arabic definition: {}", e);
        }
    }
    
    // Test caching loader
    println!("\nTesting caching loader...");
    let mut loader = LanguageLoader::new();
    
    match loader.load_language("languages/en.toml") {
        Ok(definition) => {
            println!("First load successful: {}", definition.name);
        }
        Err(e) => {
            println!("First load failed: {}", e);
        }
    }
    
    match loader.load_language("languages/en.toml") {
        Ok(definition) => {
            println!("Cached load successful: {}", definition.name);
        }
        Err(e) => {
            println!("Cached load failed: {}", e);
        }
    }
    
    let cache_stats = loader.cache_stats();
    println!("Cache stats: {} languages, {} bytes", 
             cache_stats.cached_languages, cache_stats.total_memory_usage);
    
    println!("\nLanguage loading test complete!");
}