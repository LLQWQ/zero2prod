use std::net::TcpListener;

use once_cell::sync::Lazy;
use secrecy::{ExposeSecret, SecretString};
use sqlx::{Connection, Executor, PgConnection, PgPool};
use uuid::Uuid;
use zero2prod::{
    configuration::{get_config, DatabaseSettings},
    telemetry::{get_subscriber, init_subscriber},
};

struct TestApp {
    address: String,
    db_pool: PgPool,
}

#[tokio::test]
async fn healtch_check_success() {
    let TestApp {
        address,
        db_pool: _,
    } = spawn_app().await;

    let client = reqwest::Client::new();

    let resp = client
        .get(&format!("{}/health_check", &address))
        .send()
        .await
        .expect("请求失败");

    assert!(resp.status().is_success());
    assert_eq!(Some(0), resp.content_length());
}

#[tokio::test]
async fn subscribe_return_a_200_for_valid_form_data() {
    let TestApp { address, db_pool } = spawn_app().await;

    let client = reqwest::Client::new();

    let body = "name=hello%20world&email=hubin-ll%40qq.com";
    let resp = client
        .post(&format!("{}/subscribe", &address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("请求失败");

    assert_eq!(200, resp.status().as_u16());

    let saved = sqlx::query!("select name, email from subscriptions",)
        .fetch_one(&db_pool)
        .await
        .expect("无法获取已保存的订阅者");

    assert_eq!(saved.email, "hubin-ll@qq.com");
    assert_eq!(saved.name, "hello world");
}

#[tokio::test]
async fn subscribe_return_a_400_when_data_is_missing() {
    let bodys = vec![
        ("email=hubin-ll%40qq.com", "姓名为空"),
        ("name=hello%20world", "邮件为空"),
        ("", "姓名和邮件都为空"),
    ];

    let TestApp {
        address,
        db_pool: _,
    } = spawn_app().await;

    let client = reqwest::Client::new();

    for (body, error_msg) in bodys {
        let resp = client
            .post(&format!("{}/subscribe", &address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("请求失败");

        assert_eq!(
            400,
            resp.status().as_u16(),
            "状态码非400,错误请求为：{}",
            error_msg
        );
    }
}

static TRACING: Lazy<()> = Lazy::new(|| {
    let default_filter_level = "info".to_string();
    let subscriber_name = "test".to_string();
    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::sink);
        init_subscriber(subscriber);
    };
});

async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);

    let tcp_listener = TcpListener::bind("localhost:0").unwrap();
    let port = tcp_listener.local_addr().unwrap().port();

    let mut settings = get_config().expect("无法读取配置");

    settings.database.database_name = Uuid::new_v4().to_string();
    let pg_pool = configure_database(&settings.database).await;

    tokio::spawn(zero2prod::startup::run(tcp_listener, pg_pool.clone()).expect("无法绑定地址"));

    TestApp {
        address: format!("http://localhost:{}", port),
        db_pool: pg_pool,
    }
}

pub async fn configure_database(config: &DatabaseSettings) -> PgPool {
    // Create database
    let maintenance_settings = DatabaseSettings {
        database_name: "postgres".to_string(),
        username: "postgres".to_string(),
        password: SecretString::from("password"),
        ..config.clone()
    };
    let mut connection =
        PgConnection::connect(&maintenance_settings.connection_string().expose_secret())
            .await
            .expect("Failed to connect to Postgres");

    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
        .await
        .expect("Failed to create database.");

    // Migrate database
    let connection_pool = PgPool::connect(&config.connection_string().expose_secret())
        .await
        .expect("Failed to connect to Postgres.");
    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate the database");
    connection_pool
}
