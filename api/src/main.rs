use foreign::ForeignType;
use jsonwrap::{Foreign, ForeignOpt};
use poem::{Route, Server, listener::TcpListener};
use poem_openapi::{OpenApi, OpenApiService, payload::Json};

struct Api;

#[OpenApi]
impl Api {
    #[oai(path = "/hello", method = "get")]
    async fn hello(&self) -> Json<Foreign<ForeignType>> {
        Json(Foreign(ForeignType {
            text: "hello".to_string(),
        }))
    }

    #[oai(path = "/optional", method = "get")]
    async fn optional(&self) -> Json<Option<Foreign<ForeignType>>> {
        Json(Some(Foreign(ForeignType {
            text: "optional value".to_string(),
        })))
    }

    #[oai(path = "/optional-none", method = "get")]
    async fn optional_none(&self) -> Json<Option<Foreign<ForeignType>>> {
        Json(None)
    }

    #[oai(path = "/foreign-opt", method = "get")]
    async fn foreign_opt(&self) -> Json<ForeignOpt<ForeignType>> {
        Json(ForeignOpt(Some(ForeignType {
            text: "using ForeignOpt".to_string(),
        })))
    }

    #[oai(path = "/foreign-opt-none", method = "get")]
    async fn foreign_opt_none(&self) -> Json<ForeignOpt<ForeignType>> {
        Json(ForeignOpt(None))
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
