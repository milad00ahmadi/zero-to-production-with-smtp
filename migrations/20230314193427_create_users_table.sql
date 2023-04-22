CREATE TABLE users(
    user_id uuid PRIMARY KEY,
    username TEXT NULL UNIQUE,
    password_hash TEXT NOT NULL
);