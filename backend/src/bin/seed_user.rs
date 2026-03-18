use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: seed_user <username> <password>");
        std::process::exit(1);
    }

    let username = &args[1];
    let password = &args[2];

    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .expect("Failed to connect to database");

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .expect("Failed to hash password")
        .to_string();

    sqlx::query(
        "INSERT INTO users (username, password_hash) VALUES ($1, $2) \
         ON CONFLICT (username) DO UPDATE SET password_hash = $2, updated_at = NOW()",
    )
    .bind(username)
    .bind(&password_hash)
    .execute(&pool)
    .await
    .expect("Failed to insert user");

    println!("User '{}' created/updated successfully.", username);
}
