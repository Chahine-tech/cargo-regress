// Intentional derive bloat: large structs with Debug derive generate heavy
// fmt::Debug implementations that inflate the .text section.
#[derive(Debug, Clone, PartialEq)]
struct AppConfig {
    host: String,
    port: u16,
    timeout_ms: u64,
    max_connections: u32,
    tls_enabled: bool,
    cert_path: String,
    key_path: String,
    log_level: String,
    db_url: String,
    redis_url: String,
    api_key: String,
    secret: String,
}

#[derive(Debug, Clone, PartialEq)]
struct DbConfig {
    url: String,
    pool_size: u32,
    idle_timeout: u64,
    max_lifetime: u64,
    statement_timeout: u64,
    schema: String,
    username: String,
    password: String,
}

#[derive(Debug, Clone, PartialEq)]
struct RedisConfig {
    url: String,
    max_connections: u32,
    connection_timeout: u64,
    read_timeout: u64,
    write_timeout: u64,
    namespace: String,
}

fn main() {
    let cfg = AppConfig {
        host: "localhost".to_string(),
        port: 8080,
        timeout_ms: 5000,
        max_connections: 100,
        tls_enabled: false,
        cert_path: "/etc/certs/cert.pem".to_string(),
        key_path: "/etc/certs/key.pem".to_string(),
        log_level: "info".to_string(),
        db_url: "postgres://localhost/mydb".to_string(),
        redis_url: "redis://localhost:6379".to_string(),
        api_key: "secret".to_string(),
        secret: "another_secret".to_string(),
    };
    println!("{cfg:?}");
}
