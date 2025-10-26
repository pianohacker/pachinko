use actix_rt;
use actix_web::{
    get,
    http::{self, StatusCode},
    post, web, App, HttpResponse, HttpServer, Responder,
};
use clap::Args;
use qualia::{object, Object, Queryable};
use serde::Deserialize;
use serde_json::json;

use crate::{
    types::{Item, Location},
    utils::choose_bin,
    CommonOpts, WithCommonOpts,
};

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

#[get("/locations")]
async fn get_locations(opts: web::Data<ApiOpts>) -> Result<impl Responder> {
    let store = opts.common.open_store()?;

    let response = web::Json(
        store
            .query(Location::q())
            .iter_converted::<Location>(&store)?
            .collect::<Vec<_>>(),
    );

    Ok(response)
}

#[derive(Debug, Deserialize)]
struct ItemCreateRequest {
    pub location_id: i64,
    pub bin_no: i64,
    pub name: String,
    pub size: String,
}

#[post("/items")]
async fn create_item(
    opts: web::Data<ApiOpts>,
    body: web::Json<ItemCreateRequest>,
) -> Result<impl Responder> {
    let mut store = opts.common.open_store()?;

    let location = match store
        .query(Location::q().id(body.location_id))
        .iter_converted::<Location>(&store)?
        .next()
    {
        None => return Ok(HttpResponse::NotFound().json(qualia::Object::new())),
        Some(i) => i,
    };

    let checkpoint = store.checkpoint()?;

    let mut item = Item {
        object_id: None,
        location: location,
        bin_no: body.bin_no,
        name: body.name.clone(),
        size: body.size.clone(),
        rest: object!(),
    };

    checkpoint.add_with_id(&mut item)?;

    checkpoint.commit(format!("update item via HTTP API: {}", item.name))?;

    Ok(HttpResponse::Ok().json(json!({
        "object_id": item.object_id
    })))
}

#[derive(Debug, Deserialize)]
struct ItemUpdateRequest {
    pub location_id: Option<i64>,
    pub bin_no: Option<i64>,
    pub name: Option<String>,
    pub size: Option<String>,
}

#[post("/items/{id}")]
async fn update_item(
    opts: web::Data<ApiOpts>,
    path: web::Path<(i64,)>,
    body: web::Json<ItemUpdateRequest>,
) -> Result<impl Responder> {
    let id = path.into_inner().0;
    let mut store = opts.common.open_store()?;

    let item = match store
        .query(Item::q().id(id))
        .iter_converted::<Item>(&store)?
        .next()
    {
        None => return Ok(HttpResponse::NotFound().json(json!({}))),
        Some(i) => i,
    };

    let checkpoint = store.checkpoint()?;

    let mut update = qualia::Object::new();
    if let Some(location_id) = body.location_id {
        update.insert("location_id".to_string(), location_id.into());
    }
    if let Some(bin_no) = body.bin_no {
        update.insert("bin_no".to_string(), bin_no.into());
    }
    if let Some(ref name) = body.name {
        update.insert("name".to_string(), name.into());
    }
    if let Some(ref size) = body.size {
        update.insert("size".to_string(), size.into());
    }

    let num_updated = checkpoint.query(Item::q().id(id)).set(update)?;

    if num_updated == 0 {
        return Ok(HttpResponse::NotFound().json(qualia::Object::new()));
    }

    checkpoint.commit(format!("update item via HTTP API: {}", item.name))?;

    Ok(HttpResponse::Ok().json(json!({})))
}

#[get("/locations/{id}/next-item-bin")]
async fn get_location_next_item_bin(
    opts: web::Data<ApiOpts>,
    path: web::Path<(i64,)>,
) -> Result<impl Responder> {
    let id = path.into_inner().0;
    let store = opts.common.open_store()?;

    let location = match store
        .query(Location::q().id(id))
        .iter_converted::<Location>(&store)?
        .next()
    {
        None => return Ok(HttpResponse::NotFound().json(qualia::Object::new())),
        Some(i) => i,
    };

    let bin_no = choose_bin(&store, location.object_id.unwrap(), location.num_bins)?;

    let response = web::Json(json!({"bin_no": bin_no}));

    Ok(HttpResponse::Ok().json(response))
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
                .service(get_locations)
                .service(get_location_next_item_bin)
                .service(create_item)
                .service(update_item)
        })
        .bind(("localhost", port))?
        .run()
        .await
    })?;

    Ok(())
}
