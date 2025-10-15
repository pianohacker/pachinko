use actix_rt;
use actix_web::{
    get,
    http::{self, StatusCode},
    web, App, HttpResponse, HttpServer, Responder,
};
use clap::Args;
use qualia::Queryable;
use serde::Deserialize;

use crate::{types::Item, CommonOpts, WithCommonOpts};

#[derive(Args, Clone)]
pub struct ApiOpts {
    #[clap(flatten)]
    common: CommonOpts,
    #[clap(short, default_value = "7224")]
    port: u16,
}

impl WithCommonOpts for ApiOpts {
    fn common_opts(&self) -> &CommonOpts {
        &self.common
    }
}

#[derive(thiserror::Error, Debug)]
enum Error {
    #[error("internal error")]
    InternalError(#[from] anyhow::Error),

    #[error("internal storage error")]
    InternalStorageError(#[from] qualia::StoreError),
}

impl actix_web::ResponseError for Error {
    fn status_code(&self) -> actix_web::http::StatusCode {
        match &self {
            Self::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::InternalStorageError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).body(self.to_string())
    }
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Deserialize)]
struct ItemsRequest {
    q: Option<String>,
}

#[get("/items")]
async fn get_items(
    opts: web::Data<ApiOpts>,
    params: web::Query<ItemsRequest>,
) -> Result<impl Responder> {
    let store = opts.common.open_store()?;

    let mut query = Item::q();
    if let Some(ref q) = params.q {
        query = query.like("name", q)
    }

    let response = web::Json(
        store
            .query(query)
            .iter_converted::<Item>(&store)?
            .collect::<Vec<_>>(),
    );

    Ok(response)
}

pub fn run_api(opts: ApiOpts) -> crate::AHResult<()> {
    actix_rt::System::new().block_on(async move {
        let port = opts.port;
        env_logger::init();

        HttpServer::new(move || {
            let cors = actix_cors::Cors::default()
                .allowed_origin("http://localhost:5173")
                .allowed_methods(vec!["GET", "POST"])
                .allowed_headers(vec![http::header::AUTHORIZATION, http::header::ACCEPT])
                .allowed_header(http::header::CONTENT_TYPE)
                .max_age(3600);

            App::new()
                .wrap(cors)
                .app_data(web::Data::new(opts.clone()))
                .service(get_items)
        })
        .bind(("127.0.0.1", port))?
        .run()
        .await
    })?;

    Ok(())
}
