use commands::ontology::{self, Source};

pub fn show(json: bool) -> Result<i32, String> {
    let config = ontology::load().map_err(|error| error.to_string())?;
    if json {
        let output = serde_json::to_string_pretty(&config)
            .map_err(|error| format!("cannot serialize config: {error}"))?;
        println!("{output}");
        return Ok(0);
    }

    println!("{:<12} {:<8} value", "key", "source");
    for field in config.ontology.fields() {
        let source = field.source.map_or("-", format_source);
        let value = field.value.unwrap_or_default();
        println!("{:<12} {:<8} {value}", field.key, source);
    }
    Ok(0)
}

fn format_source(source: Source) -> &'static str {
    match source {
        Source::Env => "env",
        Source::Config => "config",
        Source::Default => "default",
    }
}
