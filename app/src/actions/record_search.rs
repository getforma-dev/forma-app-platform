//! Read-only, explicit record selection for the app assistant. A search covers
//! one authorized collection and returns at most fifty matches, never a guessed
//! write target. Confirmation still uses a freshly checked exact record/version.
use super::routes;
use crate::{
    auth::Auth,
    http::{self, UniformPath},
    storage,
};
use axum::{
    extract::{Request, State},
    http::StatusCode,
    response::Response,
};
use std::sync::Arc;

pub const PATH: &str = "/v1/work/{app}/{work}/records/search";

fn query(value: Option<&str>) -> Option<(String, String)> {
    let value = value?;
    if value.len() > 4096 {
        return None;
    }
    let mut collection = None;
    let mut query = None;
    for (key, value) in url::form_urlencoded::parse(value.as_bytes()) {
        match key.as_ref() {
            "collection" if collection.is_none() => collection = Some(value.into_owned()),
            "q" if query.is_none() => query = Some(value.trim().to_owned()),
            _ => return None,
        }
    }
    let (collection, query) = (collection?, query?);
    (storage::valid_identifier(&collection)
        && (2..=200).contains(&query.chars().count())
        && !query.chars().any(char::is_control))
    .then_some((collection, query))
}

pub(super) async fn search(
    State(auth): State<Arc<Auth>>,
    UniformPath((app, work)): UniformPath<(String, String)>,
    request: Request,
) -> Response {
    if !storage::valid_identifier(&app) || !storage::valid_identifier(&work) {
        return http::not_found();
    }
    let Some((collection, query)) = query(request.uri().query()) else {
        return http::not_found();
    };
    let member = match routes::member(&auth, request.headers()).await {
        Ok(member) => member,
        Err(response) => return response,
    };
    let Some(stores) = auth.stores.as_ref() else {
        return http::not_found();
    };
    match stores
        .data
        .search_records(&member, &app, &work, &collection, &query)
        .await
    {
        Ok(listing) => {
            let mut document = routes::records_document(member.organization(), &listing);
            if let Some(records) = document["records"].as_array_mut() {
                for record in records {
                    for field in record["values"].as_array_mut().into_iter().flatten() {
                        field["display"] =
                            serde_json::Value::String(super::assistant::display(&field["value"]));
                    }
                }
            }
            routes::document(StatusCode::OK, &document)
        }
        Err(storage::Error::Denied) => http::not_found(),
        Err(_) => http::unavailable(),
    }
}

#[cfg(test)]
mod tests {
    use super::query;
    const COLLECTION: &str = "11111111-2222-4333-8444-555555555555";
    #[test]
    fn search_is_one_explicit_collection_with_bounded_literal_text() {
        assert_eq!(
            query(Some(&format!("collection={COLLECTION}&q=50%25+_match"))),
            Some((COLLECTION.to_owned(), "50% _match".to_owned()))
        );
        let unicode = "🔎".repeat(200);
        let encoded = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("collection", COLLECTION)
            .append_pair("q", &unicode)
            .finish();
        assert_eq!(
            query(Some(&encoded)),
            Some((COLLECTION.to_owned(), unicode))
        );
        for input in [
            String::new(),
            format!("collection={COLLECTION}&q=x"),
            format!("collection={COLLECTION}&q=a%00b"),
            format!("collection={COLLECTION}&q=hello&q=again"),
            format!("collection={COLLECTION}&q=hello&all=true"),
            format!("collection={COLLECTION}&q={}", "a".repeat(201)),
            "collection=foreign&q=hello".to_owned(),
        ] {
            assert_eq!(query(Some(&input)), None, "{input}");
        }
    }
}
