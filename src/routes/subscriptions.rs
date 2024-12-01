use actix_web::{web, HttpResponse};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

#[tracing::instrument(
    name = "准备执行订阅咯", 
    skip(form, pool), 
    fields(
        name=%form.name, 
        email=%form.email
    )
)]
pub async fn subscribe(form: web::Form<FormData>, pool: web::Data<PgPool>) -> HttpResponse {

    match insert_subscriber(form, pool).await 
    {
        Ok(_) => {
            tracing::info!("插入成功咯");
            HttpResponse::Ok().finish()
        }
        Err(e) => {
            tracing::error!("执行插入失败: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[tracing::instrument(
    name = "准备插入咯", 
    skip(form, pool), 
    fields(
        name=%form.name, 
        email=%form.email
    )
)]
async fn insert_subscriber(form: web::Form<FormData>, pool: web::Data<PgPool>) -> Result<(), sqlx::Error>{
    sqlx::query!(
        r#"
        insert into subscriptions(id, email, name, subscribed_at) values($1,$2,$3,$4)
        "#,
        Uuid::new_v4(),
        form.email,
        form.name,
        Utc::now(),
    )
    .execute(pool.as_ref()).await
    .map_err(|e|{
        tracing::error!("执行sql失败: {:?}", e);
            e
    })?;

    Ok(())
}