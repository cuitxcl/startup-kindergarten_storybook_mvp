use kindergarten_storybook_server::{api, commons::shared_demo_state};
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = api::router(shared_demo_state()).layer(CorsLayer::permissive());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:5150").await?;
    println!("demo api listening on http://127.0.0.1:5150");
    axum::serve(listener, app).await?;
    Ok(())
}
