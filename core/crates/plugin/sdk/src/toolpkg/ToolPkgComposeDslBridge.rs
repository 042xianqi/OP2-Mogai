/// Returns the lazy parser facade generated from the trusted SDK parser asset.
#[allow(non_snake_case)]
pub fn buildComposeDslParserDefinition() -> String {
    include_str!(concat!(env!("OUT_DIR"), "/compose_dsl_parser.js")).to_string()
}

/// Installs the ordered context modules assembled from compose/sources.json.
#[allow(non_snake_case)]
pub fn buildComposeDslContextBridgeDefinition() -> String {
    format!(
        "{}\n{}",
        buildComposeDslParserDefinition(),
        include_str!(concat!(env!("OUT_DIR"), "/compose_dsl_context.js"))
    )
}
