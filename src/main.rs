use axum::Router;
use axum::response::Html;
use axum::routing::get;
use axum_template::engine::Engine;
use handlebars::DirectorySourceOptions;
use handlebars::Handlebars;
use rust_embed::RustEmbed;

mod index;
mod calendar;
mod try_again;

type AppEngine = Engine<Handlebars<'static>>;

#[derive(Clone)]
pub struct AppState {
    engine: AppEngine,
}

#[cfg(debug_assertions)]
pub fn setup_handlebars(hbs: &mut Handlebars) {
    let mut dso = DirectorySourceOptions::default();
    dso.tpl_extension = "".to_string();

    hbs.set_dev_mode(true);
    hbs.register_templates_directory("./hb-templates", dso)
        .unwrap();
}

#[cfg(not(debug_assertions))]
#[derive(RustEmbed)]
#[folder = "hb-templates"]
struct Templates;

#[cfg(not(debug_assertions))]
pub fn setup_handlebars(hbs: &mut Handlebars) {
    hbs.set_dev_mode(false);
    hbs.register_embed_templates::<Templates>().unwrap();
}

pub fn render<F>(f: F) -> Html<String>
where
    F: FnOnce(&mut Vec<u8>) -> Result<(), std::io::Error>,
{
    let mut buf = Vec::new();
    f(&mut buf).expect("Error rendering template");
    let html: String = String::from_utf8_lossy(&buf).into();
    Html(html)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let _ = dotenvy::dotenv();
    let mut hbs = Handlebars::new();
    setup_handlebars(&mut hbs);
    let app = Router::new()
    .route("/", get(index::index))
    .route("/tryagain", get(try_again::try_again))
    .route("/calendar", get(calendar::calendar))
        .with_state(AppState {
            engine: Engine::from(hbs),
        });

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
