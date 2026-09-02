fn main() {
    let spec = chat_api::openapi::spec();
    println!(
        "{}",
        serde_json::to_string_pretty(&spec).expect("the OpenAPI document serialises")
    );
}
