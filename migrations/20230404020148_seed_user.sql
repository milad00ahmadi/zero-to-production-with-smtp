-- Add migration script here
INSERT INTO users (user_id, username, password_hash)
    VALUES (
            '81d7c75a-5d96-4576-b3e5-1e108d924159',
            'admin',
            '$argon2id$v=19$m=4096,t=2,p=1$q8GjvEBd+hAlJ5OMIqgk4w$pJj2FstXA5pZmsGuQ6qUIs3o6xqbnYgPa2CrmtcGNbI'
)