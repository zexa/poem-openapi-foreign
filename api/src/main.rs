use foreign::ForeignType;
use jsonwrap::Foreign;
use poem::{Route, Server, listener::TcpListener};
use poem_openapi::{OpenApi, OpenApiService, payload::Json};

struct Api;

#[OpenApi]
impl Api {
    #[oai(path = "/hello", method = "get")]
    async fn hello(&self) -> Json<Foreign<ForeignType>> {
        Json(Foreign::from(ForeignType {
            text: "hello".to_string(),
        }))
    }

    // this doesn't work well because the response isnt marked as nullable
    #[oai(path = "/optional", method = "get")]
    async fn optional(&self) -> Json<Option<Foreign<ForeignType>>> {
        Json(Some(Foreign::from(ForeignType {
            text: "optional value".to_string(),
        })))
    }

    // this doesn't work well because the response isnt marked as nullable
    #[oai(path = "/optional-none", method = "get")]
    async fn optional_none(&self) -> Json<Option<Foreign<ForeignType>>> {
        Json(None)
    }

    #[oai(path = "/foreign-opt", method = "get")]
    async fn foreign_opt(&self) -> Json<Foreign<Option<ForeignType>>> {
        Json(Foreign::from(Some(ForeignType {
            text: "using Foreign<Option<T>>".to_string(),
        })))
    }

    #[oai(path = "/foreign-opt-none", method = "get")]
    async fn foreign_opt_none(&self) -> Json<Foreign<Option<ForeignType>>> {
        Json(Foreign::from(None))
    }
}

#[tokio::main]
async fn main() {
    let api = OpenApiService::new(Api, "My API", "1.0").server("http://localhost:3000");

    let ui = api.swagger_ui(); // optional
    let spec = api.spec_endpoint();

    Server::new(TcpListener::bind("127.0.0.1:3000"))
        .run(
            Route::new()
                .nest("/", api)
                .nest("/docs", ui)
                .nest("/spec.json", spec),
        )
        .await
        .unwrap();
}
